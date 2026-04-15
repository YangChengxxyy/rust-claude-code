use std::sync::Arc;

use async_trait::async_trait;
use futures_util::future::join_all;
use futures_util::StreamExt;
use rust_claude_api::{
    ApiError, ApiMessage, ApiTool, ContentBlockAccumulator, CreateMessageRequest,
    CreateMessageResponse, MessageStream, StreamEvent,
};
use rust_claude_core::{
    message::{ContentBlock, Message, StopReason},
    permission::{PermissionCheck, PermissionRequest},
    state::AppState,
};
use rust_claude_tools::{ToolContext, ToolRegistry};
use tokio::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum QueryLoopError {
    #[error(transparent)]
    Api(#[from] ApiError),

    #[error("tool execution failed: {0}")]
    Tool(String),

    #[error("maximum query rounds exceeded")]
    MaxRoundsExceeded,
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError>;

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError>;
}

#[async_trait]
impl ModelClient for rust_claude_api::AnthropicClient {
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError> {
        rust_claude_api::AnthropicClient::create_message(self, request).await
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError> {
        rust_claude_api::AnthropicClient::create_message_stream(self, request).await
    }
}

pub struct QueryLoop<C> {
    client: C,
    tools: ToolRegistry,
    max_rounds: usize,
}

impl<C> QueryLoop<C>
where
    C: ModelClient,
{
    pub fn new(client: C, tools: ToolRegistry) -> Self {
        Self {
            client,
            tools,
            max_rounds: 8,
        }
    }

    pub fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds;
        self
    }

    pub async fn run(
        &self,
        app_state: Arc<Mutex<AppState>>,
        user_input: impl Into<String>,
    ) -> Result<Message, QueryLoopError> {
        {
            let mut state = app_state.lock().await;
            state.add_message(Message::user(user_input));
        }

        for _ in 0..self.max_rounds {
            let request = self.build_request(&app_state).await;
            let response = self.collect_response_from_stream(&request).await?;

            let assistant_message = Message::assistant(response.content.clone());
            let stop_reason = response.stop_reason.clone();

            {
                let mut state = app_state.lock().await;
                state.add_usage(&response.usage);
                state.add_message(assistant_message.clone());
            }

            if stop_reason != Some(StopReason::ToolUse) {
                return Ok(assistant_message);
            }

            self.execute_tool_uses(&app_state, &assistant_message).await?;
        }

        Err(QueryLoopError::MaxRoundsExceeded)
    }

    async fn collect_response_from_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, QueryLoopError> {
        let mut stream = self.client.create_message_stream(request).await?;
        let mut message_id = String::new();
        let response_type = "message".to_string();
        let mut role = rust_claude_core::message::Role::Assistant;
        let mut model = String::new();
        let mut content: Vec<Option<ContentBlock>> = Vec::new();
        let mut accumulators: Vec<Option<ContentBlockAccumulator>> = Vec::new();
        let mut stop_reason = None;
        let mut stop_sequence = None;
        let mut usage = None;

        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::MessageStart { message } => {
                    message_id = message.id;
                    role = message.role;
                    model = message.model;
                    usage = Some(message.usage);
                }
                StreamEvent::ContentBlockStart { index, content_block } => {
                    ensure_index(&mut content, index);
                    ensure_index(&mut accumulators, index);
                    accumulators[index] = Some(
                        ContentBlockAccumulator::from_block(&content_block)
                            .map_err(QueryLoopError::Api)?,
                    );
                    content[index] = Some(content_block);
                }
                StreamEvent::ContentBlockDelta { index, delta } => {
                    ensure_index(&mut accumulators, index);
                    let accumulator = accumulators[index]
                        .as_mut()
                        .ok_or_else(|| QueryLoopError::Tool("missing content block accumulator".to_string()))?;
                    accumulator.push(&delta).map_err(QueryLoopError::Api)?;
                }
                StreamEvent::ContentBlockStop { index } => {
                    ensure_index(&mut accumulators, index);
                    if let Some(accumulator) = accumulators[index].take() {
                        ensure_index(&mut content, index);
                        content[index] = Some(accumulator.into_content_block().map_err(QueryLoopError::Api)?);
                    }
                }
                StreamEvent::MessageDelta { delta, usage: delta_usage } => {
                    stop_reason = delta.stop_reason;
                    stop_sequence = delta.stop_sequence;
                    if delta_usage.is_some() {
                        usage = delta_usage;
                    }
                }
                StreamEvent::MessageStop => break,
                StreamEvent::Ping => {}
            }
        }

        Ok(CreateMessageResponse {
            id: message_id,
            response_type,
            role,
            content: content.into_iter().flatten().collect(),
            model,
            stop_reason,
            stop_sequence,
            usage: usage.ok_or_else(|| QueryLoopError::Tool("missing streamed usage".to_string()))?,
        })
    }

    async fn build_request(&self, app_state: &Arc<Mutex<AppState>>) -> CreateMessageRequest {
        let state = app_state.lock().await;
        let messages = state
            .messages
            .iter()
            .map(ApiMessage::from)
            .collect::<Vec<_>>();

        let tools = self
            .tools
            .list()
            .into_iter()
            .map(|tool| {
                ApiTool::new(
                    tool.info.name.clone(),
                    tool.info.description.clone(),
                    tool.info.input_schema.clone(),
                )
            })
            .collect::<Vec<_>>();

        CreateMessageRequest::new(state.session.model.clone(), messages)
            .with_system_opt(state.session.system_prompt.clone())
            .with_max_tokens(state.session.max_tokens)
            .with_tools(tools)
    }

    /// Check permission for a tool invocation. Returns `None` if allowed,
    /// or `Some((content, is_error))` if the tool should not be executed.
    fn check_tool_permission(
        app_state_snapshot: &AppState,
        tool_name: &str,
        input: &serde_json::Value,
        is_read_only: bool,
    ) -> Option<(String, bool)> {
        let command = if tool_name == "Bash" {
            input
                .get("command")
                .and_then(|v| v.as_str())
        } else {
            None
        };

        let request = PermissionRequest {
            tool_name,
            command,
            is_read_only,
        };

        match app_state_snapshot.check_permission(request) {
            PermissionCheck::Allowed => None,
            PermissionCheck::Denied { reason } => {
                Some((format!("Permission denied: {reason}"), true))
            }
            PermissionCheck::NeedsConfirmation { prompt: _ } => {
                Some((
                    "Permission denied: interactive confirmation not yet supported".to_string(),
                    true,
                ))
            }
        }
    }

    async fn execute_tool_uses(
        &self,
        app_state: &Arc<Mutex<AppState>>,
        assistant_message: &Message,
    ) -> Result<(), QueryLoopError> {
        let tool_uses = assistant_message.tool_uses();

        // Snapshot the app state for permission checks (avoids holding lock during execution).
        let state_snapshot = app_state.lock().await.clone();

        let mut concurrent = Vec::new();
        let mut serial = Vec::new();
        let mut tool_results: Vec<(usize, String, String, bool)> = Vec::new();

        for (index, (tool_use_id, name, input)) in tool_uses.into_iter().enumerate() {
            let is_read_only = self
                .tools
                .get(name)
                .map(|t| t.is_read_only)
                .unwrap_or(false);

            // Check permission before scheduling execution.
            if let Some((denial_msg, is_error)) =
                Self::check_tool_permission(&state_snapshot, name, input, is_read_only)
            {
                tool_results.push((index, tool_use_id.to_string(), denial_msg, is_error));
                continue;
            }

            let entry = (index, tool_use_id.to_string(), name.to_string(), input.clone());
            if self.tools.is_concurrency_safe(name) {
                concurrent.push(entry);
            } else {
                serial.push(entry);
            }
        }

        let concurrent_results = join_all(concurrent.into_iter().map(|(index, tool_use_id, name, input)| async move {
            let result = self
                .tools
                .execute(
                    &name,
                    input,
                    ToolContext {
                        tool_use_id,
                        app_state: Some(app_state.clone()),
                    },
                )
                .await;
            (index, result)
        }))
        .await;

        for (index, result) in concurrent_results {
            let result = result.map_err(|error| QueryLoopError::Tool(error.to_string()))?;
            tool_results.push((
                index,
                result.tool_use_id,
                result.content,
                result.is_error,
            ));
        }

        for (index, tool_use_id, name, input) in serial {
            let result = self
                .tools
                .execute(
                    &name,
                    input,
                    ToolContext {
                        tool_use_id,
                        app_state: Some(app_state.clone()),
                    },
                )
                .await
                .map_err(|error| QueryLoopError::Tool(error.to_string()))?;

            tool_results.push((
                index,
                result.tool_use_id,
                result.content,
                result.is_error,
            ));
        }

        tool_results.sort_by_key(|(index, _, _, _)| *index);

        let mut state = app_state.lock().await;
        state.add_message(Message::tool_results(
            &tool_results
                .into_iter()
                .map(|(_, tool_use_id, content, is_error)| (tool_use_id, content, is_error))
                .collect::<Vec<_>>(),
        ));
        Ok(())
    }
}

fn ensure_index<T>(items: &mut Vec<Option<T>>, index: usize) {
    while items.len() <= index {
        items.push(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::message::ContentBlock;
    use rust_claude_tools::{BashTool, FileReadTool, Tool, ToolContext, ToolError};
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    struct MockClient {
        responses: Mutex<VecDeque<CreateMessageResponse>>,
    }

    #[async_trait]
    impl ModelClient for MockClient {
        async fn create_message(
            &self,
            _request: &CreateMessageRequest,
        ) -> Result<CreateMessageResponse, ApiError> {
            let mut responses = self.responses.lock().await;
            Ok(responses.pop_front().expect("mock response should exist"))
        }

        async fn create_message_stream(
            &self,
            _request: &CreateMessageRequest,
        ) -> Result<MessageStream, ApiError> {
            let response = self.create_message(_request).await?;
            let stop_reason = response.stop_reason.clone();
            let usage = response.usage.clone();
            let message_id = response.id.clone();
            let role = response.role.clone();
            let model = response.model.clone();
            let content = response.content.clone();

            let mut events = vec![Ok(StreamEvent::MessageStart {
                message: rust_claude_api::StreamMessage {
                    id: message_id,
                    role,
                    model,
                    content: vec![],
                    stop_reason: None,
                    stop_sequence: None,
                    usage: usage.clone(),
                },
            })];

            for (index, block) in content.into_iter().enumerate() {
                let delta = match &block {
                    ContentBlock::Text { text } => Some(rust_claude_api::ContentBlockDelta::TextDelta {
                        text: text.clone(),
                    }),
                    ContentBlock::Thinking { thinking } => {
                        Some(rust_claude_api::ContentBlockDelta::ThinkingDelta {
                            thinking: thinking.clone(),
                        })
                    }
                    ContentBlock::ToolUse { .. } => None,
                    ContentBlock::ToolResult { .. } => None,
                };

                events.push(Ok(StreamEvent::ContentBlockStart {
                    index,
                    content_block: block,
                }));
                if let Some(delta) = delta {
                    events.push(Ok(StreamEvent::ContentBlockDelta { index, delta }));
                }
                events.push(Ok(StreamEvent::ContentBlockStop { index }));
            }

            events.push(Ok(StreamEvent::MessageDelta {
                delta: rust_claude_api::MessageDelta {
                    stop_reason,
                    stop_sequence: None,
                },
                usage: Some(usage),
            }));
            events.push(Ok(StreamEvent::MessageStop));

            Ok(Box::pin(futures_util::stream::iter(events)))
        }
    }

    fn usage() -> rust_claude_core::message::Usage {
        rust_claude_core::message::Usage {
            input_tokens: 1,
            output_tokens: 1,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        }
    }

    #[tokio::test]
    async fn test_query_loop_executes_tool_and_returns_final_message() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Bash",
                        serde_json::json!({ "command": "printf hello" }),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("done")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        let loop_runner = QueryLoop::new(client, tools);

        // BashTool is not read-only, so bypass permissions to let it execute.
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "say hi").await.unwrap();

        assert_eq!(final_message.content, vec![ContentBlock::text("done")]);
        let state = app_state.lock().await;
        assert_eq!(state.messages.len(), 4);
        assert!(matches!(state.messages[2].content[0], ContentBlock::ToolResult { .. }));
    }

    #[tokio::test]
    async fn test_query_loop_forwards_system_prompt() {
        struct CaptureClient {
            captured: Mutex<Vec<CreateMessageRequest>>,
        }

        #[async_trait]
        impl ModelClient for CaptureClient {
            async fn create_message(
                &self,
                request: &CreateMessageRequest,
            ) -> Result<CreateMessageResponse, ApiError> {
                self.captured.lock().await.push(request.clone());
                unreachable!("stream path should be used")
            }

            async fn create_message_stream(
                &self,
                request: &CreateMessageRequest,
            ) -> Result<MessageStream, ApiError> {
                self.captured.lock().await.push(request.clone());
                Ok(Box::pin(futures_util::stream::iter(vec![
                    Ok(StreamEvent::MessageStart {
                        message: rust_claude_api::StreamMessage {
                            id: "msg_1".to_string(),
                            role: rust_claude_core::message::Role::Assistant,
                            model: "claude-test".to_string(),
                            content: vec![],
                            stop_reason: None,
                            stop_sequence: None,
                            usage: usage(),
                        },
                    }),
                    Ok(StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: ContentBlock::text("ok"),
                    }),
                    Ok(StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: rust_claude_api::ContentBlockDelta::TextDelta {
                            text: "ok".to_string(),
                        },
                    }),
                    Ok(StreamEvent::ContentBlockStop { index: 0 }),
                    Ok(StreamEvent::MessageDelta {
                        delta: rust_claude_api::MessageDelta {
                            stop_reason: Some(StopReason::EndTurn),
                            stop_sequence: None,
                        },
                        usage: Some(usage()),
                    }),
                    Ok(StreamEvent::MessageStop),
                ])))
            }
        }

        let client = CaptureClient {
            captured: Mutex::new(Vec::new()),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools);
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.system_prompt = Some("You are concise".to_string());
        let app_state = Arc::new(Mutex::new(state));

        let _ = loop_runner.run(app_state.clone(), "hello").await.unwrap();

        let captured = loop_runner.client.captured.lock().await;
        assert_eq!(captured.len(), 1);
        assert!(matches!(
            captured[0].system.as_ref(),
            Some(rust_claude_api::SystemPrompt::Text(text)) if text == "You are concise"
        ));
    }

    #[tokio::test]
    async fn test_query_loop_supports_multiple_tool_rounds() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Bash",
                        serde_json::json!({ "command": "printf one" }),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_2",
                        "Bash",
                        serde_json::json!({ "command": "printf two" }),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_3".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("complete")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        let loop_runner = QueryLoop::new(client, tools).with_max_rounds(4);

        // BashTool is not read-only, so bypass permissions to let it execute.
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "multi").await.unwrap();

        assert_eq!(final_message.content, vec![ContentBlock::text("complete")]);
        let state = app_state.lock().await;
        assert_eq!(state.messages.len(), 6);
    }

    #[tokio::test]
    async fn test_query_loop_registers_concurrency_safe_tool() {
        let mut tools = ToolRegistry::new();
        tools.register(FileReadTool::new());
        let tool = tools.get("FileRead").unwrap();
        assert!(tool.is_concurrency_safe);
        assert!(tool.is_read_only);
    }

    struct SlowReadTool;

    struct SlowWriteTool;

    #[async_trait]
    impl Tool for SlowReadTool {
        fn info(&self) -> rust_claude_core::tool_types::ToolInfo {
            rust_claude_core::tool_types::ToolInfo {
                name: "SlowRead".to_string(),
                description: "slow read".to_string(),
                input_schema: serde_json::json!({}),
            }
        }

        fn is_read_only(&self) -> bool {
            true
        }

        fn is_concurrency_safe(&self) -> bool {
            true
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            context: ToolContext,
        ) -> Result<rust_claude_core::tool_types::ToolResult, ToolError> {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(rust_claude_core::tool_types::ToolResult::success(
                context.tool_use_id,
                "ok",
            ))
        }
    }

    #[async_trait]
    impl Tool for SlowWriteTool {
        fn info(&self) -> rust_claude_core::tool_types::ToolInfo {
            rust_claude_core::tool_types::ToolInfo {
                name: "SlowWrite".to_string(),
                description: "slow write".to_string(),
                input_schema: serde_json::json!({}),
            }
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            context: ToolContext,
        ) -> Result<rust_claude_core::tool_types::ToolResult, ToolError> {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(rust_claude_core::tool_types::ToolResult::success(
                context.tool_use_id,
                "ok",
            ))
        }
    }

    #[tokio::test]
    async fn test_query_loop_executes_concurrency_safe_tools_in_parallel() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![
                        ContentBlock::tool_use("tool_1", "SlowRead", serde_json::json!({})),
                        ContentBlock::tool_use("tool_2", "SlowRead", serde_json::json!({})),
                    ],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("done")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(SlowReadTool);
        let loop_runner = QueryLoop::new(client, tools);
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));

        let started = Instant::now();
        let _ = loop_runner.run(app_state, "parallel").await.unwrap();
        let elapsed = started.elapsed();

        assert!(elapsed < Duration::from_millis(90));
    }

    #[tokio::test]
    async fn test_query_loop_executes_non_concurrency_safe_tools_serially() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![
                        ContentBlock::tool_use("tool_1", "SlowWrite", serde_json::json!({})),
                        ContentBlock::tool_use("tool_2", "SlowWrite", serde_json::json!({})),
                    ],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("done")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(SlowWriteTool);
        let loop_runner = QueryLoop::new(client, tools);

        // SlowWriteTool is not read-only, so we need BypassPermissions to let it execute.
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        let app_state = Arc::new(Mutex::new(state));

        let started = Instant::now();
        let _ = loop_runner.run(app_state, "serial").await.unwrap();
        let elapsed = started.elapsed();

        assert!(elapsed >= Duration::from_millis(90));
    }

    #[tokio::test]
    async fn test_query_loop_consumes_streamed_response() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![CreateMessageResponse {
                id: "msg_1".to_string(),
                response_type: "message".to_string(),
                role: rust_claude_core::message::Role::Assistant,
                content: vec![ContentBlock::text("streamed")],
                model: "claude-test".to_string(),
                stop_reason: Some(StopReason::EndTurn),
                stop_sequence: None,
                usage: usage(),
            }])),
        };

        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools);
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));

        let final_message = loop_runner.run(app_state, "hello").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("streamed")]);
    }

    // --- Permission integration tests ---

    #[tokio::test]
    async fn test_query_loop_denied_tool_returns_error_tool_result() {
        // Use Plan mode which denies write (non-read-only) tools.
        // BashTool.is_read_only() == false, so Plan mode should deny it.
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Bash",
                        serde_json::json!({ "command": "echo hello" }),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("understood")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::Plan;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "run bash").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("understood")]);

        // Verify the tool result message is an error containing the denial reason.
        let state = app_state.lock().await;
        // Messages: [user, assistant(tool_use), user(tool_result), assistant(text)]
        assert_eq!(state.messages.len(), 4);
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(*is_error);
                assert!(content.contains("Permission denied"));
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_query_loop_allowed_tool_executes_normally() {
        // BypassPermissions mode allows everything.
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Bash",
                        serde_json::json!({ "command": "printf allowed" }),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("done")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "test").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("done")]);

        // Verify the tool result was successful (not an error).
        let state = app_state.lock().await;
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(!*is_error);
                assert_eq!(content, "allowed");
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_query_loop_plan_mode_denies_write_tools() {
        // SlowWriteTool is not read-only, Plan mode should deny it.
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "SlowWrite",
                        serde_json::json!({}),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("ok")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(SlowWriteTool);
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::Plan;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "write").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("ok")]);

        let state = app_state.lock().await;
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(*is_error);
                assert!(content.contains("Permission denied"));
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_query_loop_plan_mode_allows_read_tools() {
        // SlowReadTool is read-only, Plan mode should allow it.
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "SlowRead",
                        serde_json::json!({}),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("ok")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(SlowReadTool);
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::Plan;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "read").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("ok")]);

        let state = app_state.lock().await;
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(!*is_error);
                assert_eq!(content, "ok");
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_query_loop_default_mode_needs_confirmation_auto_denies() {
        // Default mode for non-read-only tools returns NeedsConfirmation,
        // which the query loop auto-denies.
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "SlowWrite",
                        serde_json::json!({}),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("ok")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(SlowWriteTool);
        let loop_runner = QueryLoop::new(client, tools);

        // Default mode: non-read-only tools need confirmation
        let state = AppState::new(std::path::PathBuf::from("/tmp"));
        let app_state = Arc::new(Mutex::new(state));

        let _ = loop_runner.run(app_state.clone(), "write").await.unwrap();

        let state = app_state.lock().await;
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(*is_error);
                assert!(content.contains("interactive confirmation not yet supported"));
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_query_loop_deny_rule_blocks_specific_bash_command() {
        // Test that a deny rule on a specific bash command pattern works.
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Bash",
                        serde_json::json!({ "command": "git push" }),
                    )],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::ToolUse),
                    stop_sequence: None,
                    usage: usage(),
                },
                CreateMessageResponse {
                    id: "msg_2".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("ok")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        state.always_deny_rules = vec![rust_claude_core::permission::PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git push".to_string()),
            rule_type: rust_claude_core::permission::RuleType::Deny,
        }];
        let app_state = Arc::new(Mutex::new(state));

        let _ = loop_runner.run(app_state.clone(), "push").await.unwrap();

        let state = app_state.lock().await;
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(*is_error);
                assert!(content.contains("Permission denied"));
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }
}
