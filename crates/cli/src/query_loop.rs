use std::sync::Arc;

use futures_util::future::join_all;
use futures_util::StreamExt;
use rust_claude_api::{
    inject_cache_control_on_messages, ApiError, ApiMessage, ApiTool, ContentBlockAccumulator,
    CreateMessageRequest, CreateMessageResponse, ModelClient, StreamEvent, SystemBlock,
    SystemPrompt,
};
use rust_claude_core::{
    compaction::CompactionConfig,
    memory,
    message::{ContentBlock, Message, StopReason},
    model::{
        get_runtime_main_loop_model, get_thinking_config_for_model, normalize_model_string_for_api,
        usage_exceeds_200k_tokens,
    },
    permission::{PermissionCheck, PermissionRequest},
    state::AppState,
};
use rust_claude_tools::{ToolContext, ToolRegistry};
use tokio::sync::Mutex;

use crate::hooks::HookRunner;

#[derive(Debug, thiserror::Error)]
pub enum QueryLoopError {
    #[error(transparent)]
    Api(#[from] ApiError),

    #[error("tool execution failed: {0}")]
    Tool(String),

    #[error("maximum query rounds exceeded")]
    MaxRoundsExceeded,
}

const MAX_TOKENS_RECOVERY_LIMIT: usize = 3;
const MAX_TOKENS_RECOVERY_MESSAGE: &str =
    "Continue from where you left off. Do not repeat what you already said. Pick up mid-thought if needed. Break remaining work into smaller pieces.";

pub struct QueryLoop<C> {
    client: C,
    tools: ToolRegistry,
    max_rounds: usize,
    bridge: Option<rust_claude_tui::TuiBridge>,
    compaction_config: Option<CompactionConfig>,
    hook_runner: Option<Arc<HookRunner>>,
    agent_context: Option<rust_claude_tools::AgentContext>,
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
            bridge: None,
            compaction_config: None,
            hook_runner: None,
            agent_context: None,
        }
    }

    pub fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds;
        self
    }

    pub fn with_bridge(mut self, bridge: rust_claude_tui::TuiBridge) -> Self {
        self.bridge = Some(bridge);
        self
    }

    pub fn with_hook_runner(mut self, runner: Arc<HookRunner>) -> Self {
        self.hook_runner = Some(runner);
        self
    }

    pub fn with_compaction_config(mut self, config: CompactionConfig) -> Self {
        self.compaction_config = Some(config);
        self
    }

    pub fn with_agent_context(mut self, ctx: rust_claude_tools::AgentContext) -> Self {
        self.agent_context = Some(ctx);
        self
    }

    pub async fn run(
        &self,
        app_state: Arc<Mutex<AppState>>,
        user_input: impl Into<String>,
    ) -> Result<Message, QueryLoopError> {
        let user_input_str = user_input.into();

        // Fire UserPromptSubmit hooks
        if let Some(runner) = &self.hook_runner {
            runner.run_user_prompt_submit(&user_input_str, "").await;
        }

        // Discover & scan the memory store once at the start rather than on
        // every single agentic round.
        let scanned_memory = {
            let cwd = app_state.lock().await.cwd.clone();
            memory::discover_memory_store(&cwd)
                .and_then(|store| memory::scan_memory_store(&store).ok())
        };

        {
            let mut state = app_state.lock().await;
            state.add_message(Message::user(user_input_str));
        }

        let mut max_tokens_recovery_count: usize = 0;

        for _ in 0..self.max_rounds {
            // Auto-compaction check before building the request
            if let Some(compaction_config) = &self.compaction_config {
                let service = crate::compaction::CompactionService::new(
                    &self.client,
                    compaction_config.clone(),
                );
                match service.compact_if_needed(&app_state).await {
                    Ok(Some(result)) => {
                        if let Some(bridge) = &self.bridge {
                            bridge.send_compaction_start().await;
                            bridge.send_compaction_complete(result).await;
                        }
                    }
                    Ok(None) => {} // no compaction needed
                    Err(e) => {
                        if let Some(bridge) = &self.bridge {
                            bridge
                                .send_error(&format!("Auto-compaction failed: {e}"))
                                .await;
                        }
                    }
                }
            }

            let request = self
                .build_request(&app_state, scanned_memory.as_ref())
                .await;
            let use_stream = {
                let state = app_state.lock().await;
                state.session.stream
            };
            let response = if use_stream {
                self.collect_response_from_stream(&request).await?
            } else {
                self.client.create_message(&request).await?
            };

            let assistant_message =
                Message::assistant_with_usage(response.content.clone(), response.usage.clone());

            let stop_reason = response.stop_reason.clone();

            // Notify bridge that streaming ended
            if let Some(bridge) = &self.bridge {
                bridge.send_stream_end().await;
                let usage = &response.usage;
                bridge
                    .send_usage_update(
                        usage.input_tokens as u64,
                        usage.output_tokens as u64,
                        usage.cache_read_input_tokens as u64,
                        usage.cache_creation_input_tokens as u64,
                    )
                    .await;
            }

            {
                let mut state = app_state.lock().await;
                state.add_assistant_message(assistant_message.clone());
                // Track API usage for usage-based token counting
                state.update_api_usage(response.usage.clone());
            }

            // Handle max tokens truncation with recovery
            if stop_reason == Some(StopReason::MaxTokens) {
                // If the response has tool_use blocks, execute them first
                if assistant_message.has_tool_use() {
                    self.execute_tool_uses(&app_state, &assistant_message)
                        .await?;
                }

                if max_tokens_recovery_count < MAX_TOKENS_RECOVERY_LIMIT {
                    max_tokens_recovery_count += 1;

                    if let Some(bridge) = &self.bridge {
                        bridge
                            .send_error(&format!(
                                "Output truncated, continuing... (attempt {}/{})",
                                max_tokens_recovery_count, MAX_TOKENS_RECOVERY_LIMIT
                            ))
                            .await;
                    }

                    // Inject continuation message
                    {
                        let mut state = app_state.lock().await;
                        state.add_message(Message::user(MAX_TOKENS_RECOVERY_MESSAGE));
                    }
                    continue;
                }

                // Exhausted recovery attempts, return truncated result
                if let Some(runner) = &self.hook_runner {
                    runner.run_stop("max_tokens", "").await;
                }
                return Ok(assistant_message);
            }

            // Reset recovery counter on successful non-MaxTokens response
            max_tokens_recovery_count = 0;

            if stop_reason != Some(StopReason::ToolUse) {
                if let Some(runner) = &self.hook_runner {
                    runner.run_stop("end_turn", "").await;
                }
                return Ok(assistant_message);
            }

            self.execute_tool_uses(&app_state, &assistant_message)
                .await?;
        }

        if let Some(runner) = &self.hook_runner {
            runner.run_stop("max_rounds", "").await;
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
                    if let Some(bridge) = &self.bridge {
                        bridge.send_stream_start().await;
                    }
                    message_id = message.id;
                    role = message.role;
                    model = message.model;
                    usage = Some(message.usage);
                }
                StreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => {
                    ensure_index(&mut content, index);
                    ensure_index(&mut accumulators, index);
                    // Forward tool_use block starts to the bridge for streaming display
                    if let Some(bridge) = &self.bridge {
                        if let ContentBlock::ToolUse { name, .. } = &content_block {
                            bridge.send_tool_input_stream_start(name).await;
                        }
                    }
                    accumulators[index] = Some(
                        ContentBlockAccumulator::from_block(&content_block)
                            .map_err(QueryLoopError::Api)?,
                    );
                    content[index] = Some(content_block);
                }
                StreamEvent::ContentBlockDelta { index, delta } => {
                    ensure_index(&mut accumulators, index);
                    let accumulator = accumulators[index].as_mut().ok_or_else(|| {
                        QueryLoopError::Tool("missing content block accumulator".to_string())
                    })?;
                    // Send streaming deltas to the TUI bridge
                    if let Some(bridge) = &self.bridge {
                        match &delta {
                            rust_claude_api::ContentBlockDelta::TextDelta { text } => {
                                bridge.send_stream_delta(text).await;
                            }
                            rust_claude_api::ContentBlockDelta::ThinkingDelta { thinking } => {
                                bridge.send_thinking_start().await;
                                bridge.send_thinking_delta(thinking).await;
                            }
                            rust_claude_api::ContentBlockDelta::InputJsonDelta { partial_json } => {
                                // Resolve tool name from the content block at this index
                                let tool_name = content[index]
                                    .as_ref()
                                    .and_then(|block| {
                                        if let ContentBlock::ToolUse { name, .. } = block {
                                            Some(name.as_str())
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or("unknown");
                                bridge.send_tool_input_delta(tool_name, partial_json).await;
                            }
                            _ => {}
                        }
                    }
                    accumulator.push(&delta).map_err(QueryLoopError::Api)?;
                }
                StreamEvent::ContentBlockStop { index } => {
                    ensure_index(&mut accumulators, index);
                    if let Some(accumulator) = accumulators[index].take() {
                        ensure_index(&mut content, index);
                        let block = accumulator
                            .into_content_block()
                            .map_err(QueryLoopError::Api)?;
                        if let Some(bridge) = &self.bridge {
                            if let ContentBlock::Thinking { thinking, .. } = &block {
                                bridge.send_thinking_complete(thinking).await;
                            }
                        }
                        content[index] = Some(block);
                    }
                }
                StreamEvent::MessageDelta {
                    delta,
                    usage: delta_usage,
                } => {
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
            usage: usage
                .ok_or_else(|| QueryLoopError::Tool("missing streamed usage".to_string()))?,
        })
    }

    async fn build_request(
        &self,
        app_state: &Arc<Mutex<AppState>>,
        scanned_memory: Option<&memory::ScannedMemoryStore>,
    ) -> CreateMessageRequest {
        let state = app_state.lock().await;
        let exceeds_200k_tokens = state
            .most_recent_assistant_usage()
            .is_some_and(usage_exceeds_200k_tokens);
        let runtime_model = get_runtime_main_loop_model(
            &state.session.model_setting,
            state.permission_mode,
            exceeds_200k_tokens,
        );
        let user_text = state
            .messages
            .iter()
            .rev()
            .find_map(|msg| {
                if matches!(msg.role, rust_claude_core::message::Role::User) {
                    msg.content.iter().find_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let relevant_memories = scanned_memory
            .map(|scanned| memory::select_relevant_memories(scanned, &user_text, 5))
            .unwrap_or_default();

        // Serialize messages as JSON values so we can inject cache_control
        let messages: Vec<ApiMessage> = state.messages.iter().map(ApiMessage::from).collect();
        let mut serialized_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter_map(|m| {
                let val = serde_json::to_value(m);
                if let Err(ref e) = val {
                    eprintln!("Warning: failed to serialize message, skipping: {e}");
                }
                val.ok()
            })
            .collect();
        inject_cache_control_on_messages(&mut serialized_messages);

        // Convert system prompt to structured blocks with cache_control on last block
        let system = state.session.system_prompt.as_ref().map(|text| {
            let mut full_prompt = text.clone();
            if let Some(extra) = memory::build_relevant_memories_section(&relevant_memories) {
                if !full_prompt.is_empty() {
                    full_prompt.push_str("\n\n");
                }
                full_prompt.push_str(&extra);
            }
            SystemPrompt::StructuredBlocks(
                vec![SystemBlock::text(full_prompt).with_cache_control()],
            )
        });

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

        // Determine thinking config based on model
        let thinking_config =
            get_thinking_config_for_model(&runtime_model, state.session.thinking_enabled);
        let max_tokens = state.session.max_tokens;

        let mut request =
            CreateMessageRequest::new(normalize_model_string_for_api(&runtime_model), messages)
                .with_max_tokens(max_tokens)
                .with_tools(tools)
                .with_stream(state.session.stream);

        if let Some(system) = system {
            request.system = Some(system);
        }

        // Inject thinking config
        if let Some(thinking_value) = thinking_config.to_api_value(max_tokens) {
            request = request.with_thinking(thinking_value);
        }

        // Replace messages with cache_control-injected versions
        request.messages = serialized_messages
            .into_iter()
            .filter_map(|v| {
                let result: Result<ApiMessage, _> = serde_json::from_value(v);
                if let Err(ref e) = result {
                    eprintln!(
                        "Warning: failed to deserialize cache-injected message, skipping: {e}"
                    );
                }
                result.ok()
            })
            .collect();

        request
    }

    /// Check permission for a tool invocation. Returns `None` if allowed,
    /// or `Some((content, is_error))` if the tool should not be executed.
    /// When a TuiBridge is connected, `NeedsConfirmation` is forwarded to the
    /// user as an interactive dialog. Without a bridge it is auto-denied.
    async fn check_tool_permission(
        &self,
        app_state: &Arc<Mutex<AppState>>,
        tool_name: &str,
        input: &serde_json::Value,
        is_read_only: bool,
    ) -> Option<(String, bool)> {
        let command = if tool_name == "Bash" {
            input.get("command").and_then(|v| v.as_str())
        } else {
            None
        };

        let request = PermissionRequest {
            tool_name,
            command,
            is_read_only,
        };

        let check = {
            let state = app_state.lock().await;
            state.check_permission(request)
        };

        match check {
            PermissionCheck::Allowed => None,
            PermissionCheck::Denied { reason } => {
                Some((format!("Permission denied: {reason}"), true))
            }
            PermissionCheck::NeedsConfirmation { prompt: _ } => {
                if let Some(bridge) = &self.bridge {
                    // Send permission request to TUI and wait for response
                    match bridge.request_permission(tool_name, input).await {
                        Some(rust_claude_tui::PermissionResponse::Allow) => None,
                        Some(rust_claude_tui::PermissionResponse::AlwaysAllow) => {
                            let mut state = app_state.lock().await;
                            let rule = rust_claude_core::permission::PermissionRule {
                                tool_name: tool_name.to_string(),
                                // Use the exact command as the pattern so we
                                // don't accidentally allow broader commands
                                // than the user intended.
                                pattern: command.map(|c| c.to_string()),
                                rule_type: rust_claude_core::permission::RuleType::Allow,
                            };
                            state.always_allow_rules.push(rule);
                            None
                        }
                        Some(rust_claude_tui::PermissionResponse::Deny) => {
                            Some(("Permission denied by user".to_string(), true))
                        }
                        Some(rust_claude_tui::PermissionResponse::AlwaysDeny) => {
                            let mut state = app_state.lock().await;
                            let rule = rust_claude_core::permission::PermissionRule {
                                tool_name: tool_name.to_string(),
                                pattern: command.map(|c| c.to_string()),
                                rule_type: rust_claude_core::permission::RuleType::Deny,
                            };
                            state.always_deny_rules.push(rule);
                            Some(("Permission denied by user (always deny)".to_string(), true))
                        }
                        None => Some(("Permission denied: dialog unavailable".to_string(), true)),
                    }
                } else {
                    Some((
                        "Permission denied: interactive confirmation not yet supported".to_string(),
                        true,
                    ))
                }
            }
        }
    }

    async fn execute_tool_uses(
        &self,
        app_state: &Arc<Mutex<AppState>>,
        assistant_message: &Message,
    ) -> Result<(), QueryLoopError> {
        let tool_uses = assistant_message.tool_uses();

        let mut concurrent = Vec::new();
        let mut serial = Vec::new();
        let mut tool_results: Vec<(usize, String, String, bool)> = Vec::new();

        for (index, (tool_use_id, name, input)) in tool_uses.into_iter().enumerate() {
            let is_read_only = self
                .tools
                .get(name)
                .map(|t| t.is_read_only)
                .unwrap_or(false);

            // Notify TUI about tool use start
            if let Some(bridge) = &self.bridge {
                bridge.send_tool_use(name, input).await;
            }

            // Check permission before scheduling execution.
            if let Some((denial_msg, is_error)) = self
                .check_tool_permission(app_state, name, input, is_read_only)
                .await
            {
                if let Some(bridge) = &self.bridge {
                    bridge.send_tool_result(name, &denial_msg, is_error).await;
                }
                tool_results.push((index, tool_use_id.to_string(), denial_msg, is_error));
                continue;
            }

            // Run PreToolUse hooks after permission check passes
            if let Some(runner) = &self.hook_runner {
                let hook_result = runner.run_pre_tool_use(name, input, "").await;
                if let rust_claude_core::hooks::HookResult::Block { reason } = hook_result {
                    let msg = format!("Hook blocked: {}", reason);
                    if let Some(bridge) = &self.bridge {
                        bridge.send_hook_blocked(name, &reason).await;
                        bridge.send_tool_result(name, &msg, true).await;
                    }
                    tool_results.push((index, tool_use_id.to_string(), msg, true));
                    continue;
                }
            }

            let entry = (
                index,
                tool_use_id.to_string(),
                name.to_string(),
                input.clone(),
            );
            if self.tools.is_concurrency_safe(name) {
                concurrent.push(entry);
            } else {
                serial.push(entry);
            }
        }

        let concurrent_results = join_all(concurrent.into_iter().map(
            |(index, tool_use_id, name, input)| async move {
                let result = self
                    .tools
                    .execute(
                        &name,
                        input.clone(),
                        ToolContext {
                            tool_use_id,
                            app_state: Some(app_state.clone()),
                            agent_context: self.agent_context.clone(),
                        },
                    )
                    .await;
                (index, name, input, result)
            },
        ))
        .await;

        for (index, name, input, result) in concurrent_results {
            let result = result.map_err(|error| QueryLoopError::Tool(error.to_string()))?;
            if let Some(bridge) = &self.bridge {
                bridge
                    .send_tool_result(&name, &result.content, result.is_error)
                    .await;
            }
            // Fire PostToolUse hooks
            if let Some(runner) = &self.hook_runner {
                runner
                    .run_post_tool_use(&name, &input, &result.content, result.is_error, "")
                    .await;
            }
            tool_results.push((index, result.tool_use_id, result.content, result.is_error));
        }

        for (index, tool_use_id, name, input) in serial {
            let result = self
                .tools
                .execute(
                    &name,
                    input.clone(),
                    ToolContext {
                        tool_use_id,
                        app_state: Some(app_state.clone()),
                        agent_context: self.agent_context.clone(),
                    },
                )
                .await
                .map_err(|error| QueryLoopError::Tool(error.to_string()))?;

            if let Some(bridge) = &self.bridge {
                bridge
                    .send_tool_result(&name, &result.content, result.is_error)
                    .await;
            }
            // Fire PostToolUse hooks
            if let Some(runner) = &self.hook_runner {
                runner
                    .run_post_tool_use(&name, &input, &result.content, result.is_error, "")
                    .await;
            }
            tool_results.push((index, result.tool_use_id, result.content, result.is_error));
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
    use async_trait::async_trait;
    use rust_claude_api::MessageStream;
    use rust_claude_core::message::{ContentBlock, Message};
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
                    ContentBlock::Text { text } => {
                        Some(rust_claude_api::ContentBlockDelta::TextDelta { text: text.clone() })
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        Some(rust_claude_api::ContentBlockDelta::ThinkingDelta {
                            thinking: thinking.clone(),
                        })
                    }
                    ContentBlock::Image { .. } => None,
                    ContentBlock::ToolUse { .. } => None,
                    ContentBlock::ToolResult { .. } => None,
                    ContentBlock::Unknown => None,
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
        assert!(matches!(
            state.messages[2].content[0],
            ContentBlock::ToolResult { .. }
        ));
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
        // System prompt is now converted to StructuredBlocks with cache_control
        match captured[0].system.as_ref() {
            Some(rust_claude_api::SystemPrompt::StructuredBlocks(blocks)) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks[0].text, "You are concise");
                assert!(blocks[0].cache_control.is_some());
            }
            other => panic!("expected StructuredBlocks, got: {other:?}"),
        }
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

        let final_message = loop_runner
            .run(app_state.clone(), "run bash")
            .await
            .unwrap();
        assert_eq!(
            final_message.content,
            vec![ContentBlock::text("understood")]
        );

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
    async fn test_build_request_uses_opus_for_opusplan_in_plan_mode() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::new()),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::Plan;
        state.session.model_setting = "opusplan".to_string();
        state.session.model = "claude-sonnet-4-6".to_string();
        state.add_message(Message::user("hello"));
        let app_state = Arc::new(Mutex::new(state));

        let request = loop_runner.build_request(&app_state, None).await;
        assert_eq!(request.model, "claude-opus-4-6");
    }

    #[tokio::test]
    async fn test_build_request_uses_sonnet_for_opusplan_when_last_usage_exceeds_200k() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::new()),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::Plan;
        state.session.model_setting = "opusplan".to_string();
        state.session.model = "claude-sonnet-4-6".to_string();
        state.add_assistant_message(Message::assistant_with_usage(
            vec![ContentBlock::text("previous")],
            rust_claude_core::message::Usage {
                input_tokens: 150_000,
                output_tokens: 40_000,
                cache_creation_input_tokens: 10_001,
                cache_read_input_tokens: 0,
            },
        ));
        state.add_message(Message::user("hello"));
        let app_state = Arc::new(Mutex::new(state));

        let request = loop_runner.build_request(&app_state, None).await;
        assert_eq!(request.model, "claude-sonnet-4-6");
    }
}
