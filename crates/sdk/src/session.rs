use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::Stream;
use rust_claude_api::{AnthropicClient, ModelClient};
use rust_claude_core::{
    compaction::CompactionConfig,
    config::{Config, Provider},
    message::{ContentBlock, Message, Usage},
    permission::PermissionMode,
    state::AppState,
    tool_types::ToolResult,
};
use rust_claude_tools::{
    AgentTool, AskUserQuestionTool, AutoMemoryTool, BashTool, EnterPlanModeTool, ExitPlanModeTool,
    FileEditTool, FileReadTool, FileWriteTool, GlobTool, GrepTool, LspTool, MonitorTool,
    NotebookEditTool, SendMessageTool, SkillTool, TaskCreateTool, TaskGetTool, TaskListTool,
    TaskTool, TaskUpdateTool, TeamCreateTool, TeamDeleteTool, Tool, ToolRegistry, ToolSearchTool,
    WebFetchTool, WebSearchTool,
};
use tokio::sync::{mpsc, Mutex};

use crate::agent_loop::{QueryLoop, QueryLoopError};
use crate::hooks::HookRunner;
use crate::output::{OutputSink, PermissionUI, UserQuestionUI};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rust_claude_api::ApiError),
    #[error(transparent)]
    Agent(#[from] QueryLoopError),
    #[error("invalid SDK configuration: {0}")]
    InvalidConfig(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        name: String,
        output: String,
        is_error: bool,
    },
    Usage(Usage),
    Error(String),
    Done,
}

pub struct ResponseStream {
    receiver: mpsc::Receiver<ResponseEvent>,
}

impl Stream for ResponseStream {
    type Item = ResponseEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

#[derive(Clone)]
struct ChannelOutputSink {
    sender: mpsc::Sender<ResponseEvent>,
}

impl ChannelOutputSink {
    fn send(&self, event: ResponseEvent) {
        let _ = self.sender.try_send(event);
    }
}

impl OutputSink for ChannelOutputSink {
    fn stream_delta(&self, text: &str) {
        self.send(ResponseEvent::TextDelta(text.to_string()));
    }

    fn thinking_delta(&self, text: &str) {
        self.send(ResponseEvent::ThinkingDelta(text.to_string()));
    }

    fn tool_use(&self, name: &str, input: &serde_json::Value) {
        self.send(ResponseEvent::ToolUse {
            name: name.to_string(),
            input: input.clone(),
        });
    }

    fn tool_result(&self, name: &str, output: &str, is_error: bool) {
        self.send(ResponseEvent::ToolResult {
            name: name.to_string(),
            output: output.to_string(),
            is_error,
        });
    }

    fn usage(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    ) {
        self.send(ResponseEvent::Usage(Usage {
            input_tokens: input_tokens as u32,
            output_tokens: output_tokens as u32,
            cache_read_input_tokens: cache_read_input_tokens as u32,
            cache_creation_input_tokens: cache_creation_input_tokens as u32,
        }));
    }

    fn error(&self, message: &str) {
        self.send(ResponseEvent::Error(message.to_string()));
    }
}

pub struct Session {
    client: Arc<dyn ModelClient>,
    tools: Arc<ToolRegistry>,
    app_state: Arc<Mutex<AppState>>,
    max_rounds: usize,
    _output: Option<Box<dyn OutputSink>>,
    _permission_ui: Option<Box<dyn PermissionUI>>,
    _user_question_ui: Option<Box<dyn UserQuestionUI>>,
    hook_runner: Option<Arc<HookRunner>>,
    compaction_config: Option<CompactionConfig>,
}

impl Session {
    pub fn builder() -> SessionBuilder {
        SessionBuilder::default()
    }

    pub async fn send(&self, prompt: &str) -> Result<Message> {
        self.query_loop(None, None, None)
            .run(self.app_state.clone(), prompt)
            .await
            .map_err(Into::into)
    }

    pub async fn send_with_tools(
        &self,
        prompt: &str,
        tool_results: Vec<ToolResult>,
    ) -> Result<Message> {
        if !tool_results.is_empty() {
            let blocks = tool_results
                .into_iter()
                .map(|result| {
                    ContentBlock::tool_result(result.tool_use_id, result.content, result.is_error)
                })
                .collect();
            self.app_state
                .lock()
                .await
                .add_message(Message::user_with_blocks(blocks));
        }
        self.send(prompt).await
    }

    pub fn send_streaming(&self, prompt: &str) -> Result<ResponseStream> {
        let (tx, rx) = mpsc::channel(256);
        let done_tx = tx.clone();
        let query_loop = self.query_loop(
            Some(Box::new(ChannelOutputSink { sender: tx.clone() })),
            None,
            None,
        );
        let app_state = self.app_state.clone();
        let prompt = prompt.to_string();
        tokio::spawn(async move {
            if let Err(error) = query_loop.run(app_state, prompt).await {
                let _ = tx.send(ResponseEvent::Error(error.to_string())).await;
            }
            let _ = done_tx.send(ResponseEvent::Done).await;
        });
        Ok(ResponseStream { receiver: rx })
    }

    fn query_loop(
        &self,
        output: Option<Box<dyn OutputSink>>,
        permission_ui: Option<Box<dyn PermissionUI>>,
        user_question_ui: Option<Box<dyn UserQuestionUI>>,
    ) -> QueryLoop<Arc<dyn ModelClient>> {
        let mut loop_ = QueryLoop::new(self.client.clone(), self.tools.clone())
            .with_max_rounds(self.max_rounds);
        if let Some(config) = self.compaction_config.clone() {
            loop_ = loop_.with_compaction_config(config);
        }
        if let Some(runner) = &self.hook_runner {
            loop_ = loop_.with_hook_runner(runner.clone());
        }
        if let Some(output) = output {
            loop_ = loop_.with_output(output);
        }
        if let Some(permission_ui) = permission_ui {
            loop_ = loop_.with_permission_ui(permission_ui);
        }
        if let Some(user_question_ui) = user_question_ui {
            loop_ = loop_.with_user_question_ui(user_question_ui);
        }
        loop_
    }
}

#[derive(Default)]
pub struct SessionBuilder {
    config: Option<Config>,
    api_key: Option<String>,
    bearer_auth: bool,
    model: Option<String>,
    base_url: Option<String>,
    system_prompt: Option<String>,
    permission_mode: Option<PermissionMode>,
    max_rounds: Option<usize>,
    output: Option<Box<dyn OutputSink>>,
    permission_ui: Option<Box<dyn PermissionUI>>,
    user_question_ui: Option<Box<dyn UserQuestionUI>>,
    hook_runner: Option<Arc<HookRunner>>,
    compaction_config: Option<CompactionConfig>,
    client: Option<Arc<dyn ModelClient>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    custom_tools: Vec<Arc<dyn Tool>>,
}

impl SessionBuilder {
    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }
    pub fn bearer_auth(mut self, bearer_auth: bool) -> Self {
        self.bearer_auth = bearer_auth;
        self
    }
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = Some(mode);
        self
    }
    pub fn max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = Some(max_rounds);
        self
    }
    pub fn output_sink(mut self, output: Box<dyn OutputSink>) -> Self {
        self.output = Some(output);
        self
    }
    pub fn permission_ui(mut self, ui: Box<dyn PermissionUI>) -> Self {
        self.permission_ui = Some(ui);
        self
    }
    pub fn user_question_ui(mut self, ui: Box<dyn UserQuestionUI>) -> Self {
        self.user_question_ui = Some(ui);
        self
    }
    pub fn hooks(mut self, runner: Arc<HookRunner>) -> Self {
        self.hook_runner = Some(runner);
        self
    }
    pub fn compaction_config(mut self, config: CompactionConfig) -> Self {
        self.compaction_config = Some(config);
        self
    }
    pub fn client(mut self, client: Arc<dyn ModelClient>) -> Self {
        self.client = Some(client);
        self
    }
    pub fn tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }
    pub fn with_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.custom_tools.push(Arc::new(tool));
        self
    }
    pub fn with_tool_arc(mut self, tool: Arc<dyn Tool>) -> Self {
        self.custom_tools.push(tool);
        self
    }

    pub fn build(self) -> Result<Session> {
        let mut config = self.config.unwrap_or_else(|| {
            Config::with_credential(self.api_key.clone().unwrap_or_default(), self.bearer_auth)
        });
        if let Some(api_key) = self.api_key {
            config.api_key = api_key;
        }
        if self.bearer_auth {
            config.bearer_auth = true;
        }
        if let Some(model) = self.model {
            config.model = model;
        }
        if let Some(base_url) = self.base_url {
            config.base_url = Some(base_url);
        }
        if let Some(system_prompt) = self.system_prompt {
            config.system_prompt = Some(system_prompt);
        }
        if let Some(permission_mode) = self.permission_mode {
            config.permission_mode = permission_mode;
        }

        if config.api_key.trim().is_empty() && self.client.is_none() {
            return Err(Error::InvalidConfig(
                "api_key is required when no explicit client is provided".to_string(),
            ));
        }

        let client = match self.client {
            Some(client) => client,
            None => {
                let mut client = AnthropicClient::new(config.api_key.clone())?;
                if let Some(base_url) = &config.base_url {
                    client = client.with_base_url(base_url.clone());
                }
                if config.bearer_auth {
                    client = client.with_bearer_auth();
                }
                let _ = Provider::Anthropic;
                Arc::new(client)
            }
        };

        let tools = self.tool_registry.unwrap_or_else(default_tool_registry);
        for tool in self.custom_tools {
            let name = tool.info().name;
            if tools.contains(&name) {
                return Err(Error::InvalidConfig(format!("duplicate tool name: {name}")));
            }
            tools.register_arc(tool);
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let app_state = Arc::new(Mutex::new(AppState::from_config(cwd, &config)));

        Ok(Session {
            client,
            tools,
            app_state,
            max_rounds: self.max_rounds.unwrap_or(8),
            _output: self.output,
            _permission_ui: self.permission_ui,
            _user_question_ui: self.user_question_ui,
            hook_runner: self.hook_runner,
            compaction_config: self.compaction_config,
        })
    }
}

fn default_tool_registry() -> Arc<ToolRegistry> {
    Arc::new_cyclic(|weak| {
        let tools = ToolRegistry::new();
        tools.register(AgentTool::new());
        tools.register(AskUserQuestionTool::new());
        tools.register(AutoMemoryTool::new());
        tools.register(BashTool::new());
        tools.register(EnterPlanModeTool::new());
        tools.register(ExitPlanModeTool::new());
        tools.register(FileReadTool::new());
        tools.register(FileEditTool::new());
        tools.register(FileWriteTool::new());
        tools.register(GlobTool::new());
        tools.register(GrepTool::new());
        tools.register(LspTool::new());
        tools.register(MonitorTool::new());
        tools.register(NotebookEditTool::new());
        tools.register(TaskTool::new());
        tools.register(TaskCreateTool::new());
        tools.register(TaskGetTool::new());
        tools.register(TaskListTool::new());
        tools.register(TaskUpdateTool::new());
        tools.register(TeamCreateTool::new());
        tools.register(TeamDeleteTool::new());
        tools.register(SendMessageTool::new());
        // SkillTool: load user skills (no project dir available in the default
        // builder; the CLI path also discovers project skills).
        tools.register(SkillTool::new(std::sync::Arc::new(
            crate::skill::SkillLoader::discover(None).into_registry(),
        )));
        tools.register(WebFetchTool::new());
        tools.register(WebSearchTool::new());
        tools.register(ToolSearchTool::new(weak.clone()));
        tools
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_util::StreamExt;
    use rust_claude_api::{ApiError, CreateMessageRequest, CreateMessageResponse, MessageStream};
    use rust_claude_core::message::{Role, StopReason};
    use rust_claude_core::tool_types::ToolInfo;
    use rust_claude_tools::{ToolContext, ToolError};

    struct MockClient;

    #[async_trait]
    impl ModelClient for MockClient {
        async fn create_message(
            &self,
            _request: &CreateMessageRequest,
        ) -> std::result::Result<CreateMessageResponse, ApiError> {
            Ok(CreateMessageResponse {
                id: "msg_1".to_string(),
                response_type: "message".to_string(),
                role: Role::Assistant,
                content: vec![ContentBlock::text("ok")],
                model: "claude-test".to_string(),
                stop_reason: Some(StopReason::EndTurn),
                stop_sequence: None,
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
        }

        async fn create_message_stream(
            &self,
            _request: &CreateMessageRequest,
        ) -> std::result::Result<MessageStream, ApiError> {
            Ok(Box::pin(futures_util::stream::empty()))
        }
    }

    struct TestTool(&'static str);

    #[async_trait]
    impl Tool for TestTool {
        fn info(&self) -> ToolInfo {
            ToolInfo {
                name: self.0.to_string(),
                description: "test tool".to_string(),
                input_schema: serde_json::json!({ "type": "object" }),
            }
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            context: ToolContext,
        ) -> std::result::Result<ToolResult, ToolError> {
            Ok(ToolResult::success(context.tool_use_id, "ok".to_string()))
        }
    }

    #[test]
    fn builder_constructs_minimum_session() {
        let session = Session::builder().api_key("key").build().unwrap();
        assert_eq!(session.max_rounds, 8);
    }

    #[test]
    fn builder_accepts_full_configuration() {
        let session = Session::builder()
            .client(Arc::new(MockClient))
            .model("claude-test")
            .base_url("https://example.invalid")
            .system_prompt("system")
            .permission_mode(PermissionMode::BypassPermissions)
            .max_rounds(3)
            .compaction_config(CompactionConfig::default())
            .build()
            .unwrap();

        assert_eq!(session.max_rounds, 3);
    }

    #[test]
    fn builder_rejects_duplicate_custom_tool() {
        let result = Session::builder()
            .api_key("key")
            .with_tool(TestTool("Bash"))
            .build();
        let Err(error) = result else {
            panic!("expected duplicate tool error");
        };

        assert!(error.to_string().contains("duplicate tool name: Bash"));
    }

    #[tokio::test]
    async fn channel_output_sink_converts_events() {
        let (tx, rx) = mpsc::channel(8);
        let sink = ChannelOutputSink { sender: tx };
        sink.stream_delta("hello");
        sink.thinking_delta("think");
        sink.tool_use("Bash", &serde_json::json!({"command":"pwd"}));
        sink.tool_result("Bash", "ok", false);

        let mut stream = ResponseStream { receiver: rx };
        assert_eq!(
            stream.next().await,
            Some(ResponseEvent::TextDelta("hello".into()))
        );
        assert_eq!(
            stream.next().await,
            Some(ResponseEvent::ThinkingDelta("think".into()))
        );
        assert!(
            matches!(stream.next().await, Some(ResponseEvent::ToolUse { name, .. }) if name == "Bash")
        );
        assert!(
            matches!(stream.next().await, Some(ResponseEvent::ToolResult { name, output, is_error }) if name == "Bash" && output == "ok" && !is_error)
        );
    }
}
