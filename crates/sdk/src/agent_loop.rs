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
use rust_claude_tools::{ToolContext, ToolRegistry, UserQuestionCallback};
use tokio::sync::Mutex;

const SESSION_MCP_TOOL_LIMIT: usize = 20;
const SESSION_PERMISSION_DECISION_LIMIT: usize = 10;

use crate::hooks::HookRunner;
use crate::output::{OutputSink, PermissionDecision, PermissionUI, UserQuestionUI};

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
const MAX_OVERLOAD_RETRIES: u32 = 3;

/// Tracks retry-related state across loop iterations within a single `run()` invocation.
struct RetryState {
    /// Reactive compaction escalation stage (0 = not triggered, 1-3 = stages).
    /// Resets each turn.
    prompt_too_long_stage: u8,
    /// Number of consecutive overloaded (529) errors without a successful response.
    consecutive_overload_count: u32,
    /// Whether we switched to fallback model in this session.
    using_fallback_model: bool,
}

impl RetryState {
    fn new() -> Self {
        Self {
            prompt_too_long_stage: 0,
            consecutive_overload_count: 0,
            using_fallback_model: false,
        }
    }
}

enum ErrorRecoveryAction {
    /// Retry the current loop iteration.
    Retry,
    /// Fail with the original error.
    Fail,
}

pub struct QueryLoop<C> {
    client: C,
    tools: ToolRegistry,
    max_rounds: usize,
    output: Option<Box<dyn OutputSink>>,
    permission_ui: Option<Box<dyn PermissionUI>>,
    user_question_ui: Option<Arc<Box<dyn UserQuestionUI>>>,
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
            output: None,
            permission_ui: None,
            user_question_ui: None,
            compaction_config: None,
            hook_runner: None,
            agent_context: None,
        }
    }

    pub fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds;
        self
    }

    pub fn with_output(mut self, output: Box<dyn OutputSink>) -> Self {
        self.output = Some(output);
        self
    }

    pub fn with_permission_ui(mut self, permission_ui: Box<dyn PermissionUI>) -> Self {
        self.permission_ui = Some(permission_ui);
        self
    }

    pub fn with_user_question_ui(mut self, user_question_ui: Box<dyn UserQuestionUI>) -> Self {
        self.user_question_ui = Some(Arc::new(user_question_ui));
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

    fn user_question_callback(&self) -> Option<UserQuestionCallback> {
        let ui = Arc::clone(self.user_question_ui.as_ref()?);
        Some(Arc::new(move |request| {
            let ui = ui.clone();
            Box::pin(async move { ui.ask(request).await })
        }))
    }

    pub async fn run(
        &self,
        app_state: Arc<Mutex<AppState>>,
        user_input: impl Into<String>,
    ) -> Result<Message, QueryLoopError> {
        let user_input_str = user_input.into();
        let session_id = { app_state.lock().await.session.id.clone() };

        // Fire UserPromptSubmit hooks
        if let Some(runner) = &self.hook_runner {
            runner.run_user_prompt_submit(&user_input_str, &session_id).await;
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
        let mut retry_state = RetryState::new();

        for _ in 0..self.max_rounds {
            // Auto-compaction check before building the request
            if let Some(compaction_config) = &self.compaction_config {
                let service = crate::compaction::CompactionService::new(
                    &self.client,
                    compaction_config.clone(),
                );
                match service.compact_if_needed(&app_state).await {
                    Ok(Some(result)) => {
                        if let Some(output) = &self.output {
                            output.compaction_start();
                            output.compaction_complete(&result);
                        }
                    }
                    Ok(None) => {} // no compaction needed
                    Err(e) => {
                        if let Some(output) = &self.output {
                            output.error(&format!("Auto-compaction failed: {e}"));
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
                match self.collect_response_from_stream(&request).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        if let Some(action) = self.handle_api_error(
                            &e,
                            &mut retry_state,
                            &app_state,
                        ).await {
                            match action {
                                ErrorRecoveryAction::Retry => continue,
                                ErrorRecoveryAction::Fail => return Err(e),
                            }
                        }
                        return Err(e);
                    }
                }
            } else {
                match self.client.create_message(&request).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        let e = QueryLoopError::Api(e);
                        if let Some(action) = self.handle_api_error(
                            &e,
                            &mut retry_state,
                            &app_state,
                        ).await {
                            match action {
                                ErrorRecoveryAction::Retry => continue,
                                ErrorRecoveryAction::Fail => return Err(e),
                            }
                        }
                        return Err(e);
                    }
                }
            };

            // Successful API response: reset overload counter
            retry_state.consecutive_overload_count = 0;

            let assistant_message =
                Message::assistant_with_usage(response.content.clone(), response.usage.clone());

            let stop_reason = response.stop_reason.clone();

            // Notify output that streaming ended
            if let Some(output) = &self.output {
                output.stream_end();
                let usage = &response.usage;
                output.usage(
                    usage.input_tokens as u64,
                    usage.output_tokens as u64,
                    usage.cache_read_input_tokens as u64,
                    usage.cache_creation_input_tokens as u64,
                );
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

                    if let Some(output) = &self.output {
                        output.error(&format!(
                            "Output truncated, continuing... (attempt {}/{})",
                            max_tokens_recovery_count, MAX_TOKENS_RECOVERY_LIMIT
                        ));
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
                    runner.run_stop("max_tokens", &session_id).await;
                }
                return Ok(assistant_message);
            }

            // Reset recovery counter on successful non-MaxTokens response
            max_tokens_recovery_count = 0;

            if stop_reason != Some(StopReason::ToolUse) {
                if let Some(runner) = &self.hook_runner {
                    runner.run_stop("end_turn", &session_id).await;
                }
                return Ok(assistant_message);
            }

            self.execute_tool_uses(&app_state, &assistant_message)
                .await?;
        }

        if let Some(runner) = &self.hook_runner {
            runner.run_stop("max_rounds", &session_id).await;
        }
        Err(QueryLoopError::MaxRoundsExceeded)
    }

    /// Handle API errors that may be recoverable (prompt-too-long, overloaded).
    /// Returns `Some(action)` if the error was recognized, `None` if not handled.
    async fn handle_api_error(
        &self,
        error: &QueryLoopError,
        retry_state: &mut RetryState,
        app_state: &Arc<Mutex<AppState>>,
    ) -> Option<ErrorRecoveryAction> {
        match error {
            QueryLoopError::Api(ApiError::PromptTooLong(_)) => {
                Some(self.handle_prompt_too_long(retry_state, app_state).await)
            }
            QueryLoopError::Api(ApiError::Overloaded(_)) => {
                Some(self.handle_overloaded(retry_state, app_state).await)
            }
            _ => None,
        }
    }

    /// Three-stage reactive compaction for prompt-too-long errors.
    async fn handle_prompt_too_long(
        &self,
        retry_state: &mut RetryState,
        app_state: &Arc<Mutex<AppState>>,
    ) -> ErrorRecoveryAction {
        retry_state.prompt_too_long_stage += 1;

        match retry_state.prompt_too_long_stage {
            1 => {
                // Stage 1: Full LLM-based compaction
                if let Some(output) = &self.output {
                    output.compaction_start();
                }
                if let Some(compaction_config) = &self.compaction_config {
                    let service = crate::compaction::CompactionService::new(
                        &self.client,
                        compaction_config.clone(),
                    );
                    match service.force_compact(app_state).await {
                        Ok(result) => {
                            if let Some(output) = &self.output {
                                output.compaction_complete(&result);
                            }
                            return ErrorRecoveryAction::Retry;
                        }
                        Err(e) => {
                            if let Some(output) = &self.output {
                                output.error(&format!(
                                    "Reactive compaction failed: {e}. Trying micro-compaction..."
                                ));
                            }
                            // Fall through to stage 2
                            retry_state.prompt_too_long_stage = 2;
                            return self.handle_prompt_too_long_stage2(retry_state, app_state).await;
                        }
                    }
                }
                // No compaction config — try micro-compaction directly
                retry_state.prompt_too_long_stage = 2;
                self.handle_prompt_too_long_stage2(retry_state, app_state).await
            }
            2 => self.handle_prompt_too_long_stage2(retry_state, app_state).await,
            _ => {
                // Stage 3: Give up, report to user
                if let Some(output) = &self.output {
                    output.error(
                        "Prompt is too long and compaction could not reduce it enough. \
                         Try using /compact manually or starting a new conversation.",
                    );
                }
                ErrorRecoveryAction::Fail
            }
        }
    }

    /// Stage 2: Micro-compaction (strip old tool results).
    async fn handle_prompt_too_long_stage2(
        &self,
        _retry_state: &mut RetryState,
        app_state: &Arc<Mutex<AppState>>,
    ) -> ErrorRecoveryAction {
        if let Some(output) = &self.output {
            output.error("Attempting micro-compaction (clearing old tool results)...");
        }
        if let Some(compaction_config) = &self.compaction_config {
            let service = crate::compaction::CompactionService::new(
                &self.client,
                compaction_config.clone(),
            );
            match service.micro_compact(app_state).await {
                Ok(result) if result.blocks_cleared > 0 => {
                    if let Some(output) = &self.output {
                        output.error(&format!(
                            "Micro-compaction cleared {} tool result blocks (~{} tokens reduced)",
                            result.blocks_cleared, result.estimated_token_reduction
                        ));
                    }
                    return ErrorRecoveryAction::Retry;
                }
                Ok(_) => {
                    // Nothing to clear
                    if let Some(output) = &self.output {
                        output.error("Micro-compaction found no blocks to clear.");
                    }
                }
                Err(e) => {
                    if let Some(output) = &self.output {
                        output.error(&format!("Micro-compaction failed: {e}"));
                    }
                }
            }
        }
        // Micro-compaction didn't help or not available
        if let Some(output) = &self.output {
            output.error(
                "Prompt is too long and compaction could not reduce it enough. \
                 Try using /compact manually or starting a new conversation.",
            );
        }
        ErrorRecoveryAction::Fail
    }

    /// Handle overloaded (529) errors with backoff and model fallback.
    async fn handle_overloaded(
        &self,
        retry_state: &mut RetryState,
        app_state: &Arc<Mutex<AppState>>,
    ) -> ErrorRecoveryAction {
        retry_state.consecutive_overload_count += 1;
        let count = retry_state.consecutive_overload_count;

        if count > MAX_OVERLOAD_RETRIES && !retry_state.using_fallback_model {
            // Try to switch to fallback model
            let fallback = {
                app_state.lock().await.config.fallback_model.clone()
            };
            if let Some(fallback_model) = fallback {
                if let Some(output) = &self.output {
                    output.error(&format!(
                        "Switched to {} due to high demand",
                        fallback_model
                    ));
                }
                {
                    let mut state = app_state.lock().await;
                    state.session.model_setting = fallback_model.clone();
                    state.session.model = fallback_model;
                }
                retry_state.using_fallback_model = true;
                retry_state.consecutive_overload_count = 0;
                // Retry immediately with fallback model (no backoff)
                return ErrorRecoveryAction::Retry;
            }
        }

        // Backoff: 1s * count
        let backoff_secs = count as u64;
        if let Some(output) = &self.output {
            output.error(&format!(
                "Service overloaded. Retrying in {backoff_secs}s... (attempt {count})"
            ));
        }
        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;

        ErrorRecoveryAction::Retry
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
                    if let Some(output) = &self.output {
                        output.stream_start();
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
                    // Forward tool_use block starts to output for streaming display
                    if let Some(output) = &self.output {
                        if let ContentBlock::ToolUse { name, .. } = &content_block {
                            output.tool_input_start(name);
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
                    // Send streaming deltas to output
                    if let Some(output) = &self.output {
                        match &delta {
                            rust_claude_api::ContentBlockDelta::TextDelta { text } => {
                                output.stream_delta(text);
                            }
                            rust_claude_api::ContentBlockDelta::ThinkingDelta { thinking } => {
                                output.thinking_start();
                                output.thinking_delta(thinking);
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
                                output.tool_input_delta(tool_name, partial_json);
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
                        if let Some(output) = &self.output {
                            if let ContentBlock::Thinking { thinking, .. } = &block {
                                output.thinking_complete(thinking);
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
            get_thinking_config_for_model(&runtime_model, state.session.thinking_enabled, state.session.thinking_budget);
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
    /// When a PermissionUI is connected, `NeedsConfirmation` is forwarded to the
    /// user as an interactive dialog. Without a PermissionUI it is auto-denied.
    async fn check_tool_permission(
        &self,
        app_state: &Arc<Mutex<AppState>>,
        tool_name: &str,
        input: &serde_json::Value,
        is_read_only: bool,
    ) -> Option<(String, bool)> {
        let command = if tool_name == "Bash" || tool_name == "Monitor" {
            input.get("command").and_then(|v| v.as_str())
        } else {
            None
        };

        // Extract file path from tool input for path-based permission matching
        let extracted_path = rust_claude_core::permission::extract_file_path(tool_name, input);
        let resolved_path = extracted_path.as_ref().map(|p| {
            let path = std::path::Path::new(p);
            if path.is_absolute() {
                p.clone()
            } else {
                // Resolve relative to CWD
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(p)
                    .to_string_lossy()
                    .to_string()
            }
        });
        let request = PermissionRequest {
            tool_name,
            command,
            is_read_only,
            file_path: resolved_path.as_deref(),
        };

        let check = {
            let state = app_state.lock().await;
            state.check_permission(request)
        };

        match check {
            PermissionCheck::Allowed => {
                let mut state = app_state.lock().await;
                state.record_permission_decision(
                    tool_name,
                    "allowed",
                    command.map(|value| value.to_string()),
                    SESSION_PERMISSION_DECISION_LIMIT,
                );
                None
            }
            PermissionCheck::Denied { reason } => {
                let mut state = app_state.lock().await;
                state.record_permission_decision(
                    tool_name,
                    "denied",
                    command.map(|value| value.to_string()),
                    SESSION_PERMISSION_DECISION_LIMIT,
                );
                Some((format!("Permission denied: {reason}"), true))
            }
            PermissionCheck::NeedsConfirmation { prompt: _ } => {
                if let Some(ui) = &self.permission_ui {
                    match ui.request(tool_name, input).await {
                        Some(PermissionDecision::Allow) => {
                            let mut state = app_state.lock().await;
                            state.record_permission_decision(
                                tool_name,
                                "allowed",
                                command.map(|value| value.to_string()),
                                SESSION_PERMISSION_DECISION_LIMIT,
                            );
                            None
                        }
                        Some(PermissionDecision::AllowAlways) => {
                            let mut state = app_state.lock().await;
                            let rule = rust_claude_core::permission::PermissionRule {
                                tool_name: tool_name.to_string(),
                                // Use the exact command as the pattern so we
                                // don't accidentally allow broader commands
                                // than the user intended.
                                pattern: command.map(|c| c.to_string()),
                                path_pattern: None,
                                rule_type: rust_claude_core::permission::RuleType::Allow,
                            };
                            state.always_allow_rules.push(rule);
                            state.record_permission_decision(
                                tool_name,
                                "allowed",
                                command.map(|value| value.to_string()),
                                SESSION_PERMISSION_DECISION_LIMIT,
                            );
                            None
                        }
                        Some(PermissionDecision::Deny) => {
                            let mut state = app_state.lock().await;
                            state.record_permission_decision(
                                tool_name,
                                "denied",
                                command.map(|value| value.to_string()),
                                SESSION_PERMISSION_DECISION_LIMIT,
                            );
                            Some(("Permission denied by user".to_string(), true))
                        }
                        Some(PermissionDecision::DenyAlways) => {
                            let mut state = app_state.lock().await;
                            let rule = rust_claude_core::permission::PermissionRule {
                                tool_name: tool_name.to_string(),
                                pattern: command.map(|c| c.to_string()),
                                path_pattern: None,
                                rule_type: rust_claude_core::permission::RuleType::Deny,
                            };
                            state.always_deny_rules.push(rule);
                            state.record_permission_decision(
                                tool_name,
                                "denied",
                                command.map(|value| value.to_string()),
                                SESSION_PERMISSION_DECISION_LIMIT,
                            );
                            Some(("Permission denied by user (always deny)".to_string(), true))
                        }
                        None => {
                            let mut state = app_state.lock().await;
                            state.record_permission_decision(
                                tool_name,
                                "ask_unavailable",
                                command.map(|value| value.to_string()),
                                SESSION_PERMISSION_DECISION_LIMIT,
                            );
                            Some(("Permission denied: dialog unavailable".to_string(), true))
                        }
                    }
                } else {
                    let mut state = app_state.lock().await;
                    state.record_permission_decision(
                        tool_name,
                        "ask_unavailable",
                        command.map(|value| value.to_string()),
                        SESSION_PERMISSION_DECISION_LIMIT,
                    );
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
        let session_id = { app_state.lock().await.session.id.clone() };

        let mut concurrent = Vec::new();
        let mut serial = Vec::new();
        let mut tool_results: Vec<(usize, String, String, bool)> = Vec::new();
        let user_question_callback = self.user_question_callback();

        for (index, (tool_use_id, name, input)) in tool_uses.into_iter().enumerate() {
            let is_read_only = self
                .tools
                .get(name)
                .map(|t| t.is_read_only)
                .unwrap_or(false);

            // Notify output about tool use start
            if let Some(output) = &self.output {
                output.tool_use(name, input);
            }

            // Check permission before scheduling execution.
            if let Some((denial_msg, is_error)) = self
                .check_tool_permission(app_state, name, input, is_read_only)
                .await
            {
                if let Some(output) = &self.output {
                    output.tool_result(name, &denial_msg, is_error);
                }
                tool_results.push((index, tool_use_id.to_string(), denial_msg, is_error));
                continue;
            }

            if name.starts_with("mcp__") {
                let mut state = app_state.lock().await;
                state.record_mcp_tool_usage(name, SESSION_MCP_TOOL_LIMIT);
            }

            let mut scheduled_input = input.clone();

            // Run PreToolUse hooks after permission check passes
            if let Some(runner) = &self.hook_runner {
                let cwd = { app_state.lock().await.cwd.clone() };
                let hook_result = runner
                    .run_pre_tool_use_with_cwd(name, &scheduled_input, &session_id, &cwd)
                    .await;
                match hook_result {
                    rust_claude_core::hooks::HookResult::Continue => {}
                    rust_claude_core::hooks::HookResult::ContinueWithInput { updated_input } => {
                        scheduled_input = updated_input;
                    }
                    rust_claude_core::hooks::HookResult::Block { reason } => {
                        let msg = format!("Hook blocked: {}", reason);
                        if let Some(output) = &self.output {
                            output.hook_blocked(name, &reason);
                            output.tool_result(name, &msg, true);
                        }
                        tool_results.push((index, tool_use_id.to_string(), msg, true));
                        continue;
                    }
                }
            }

            let entry = (
                index,
                tool_use_id.to_string(),
                name.to_string(),
                scheduled_input,
            );
            if self.tools.is_concurrency_safe(name) {
                concurrent.push(entry);
            } else {
                serial.push(entry);
            }
        }

        let concurrent_results = join_all(concurrent.into_iter().map(
            |(index, tool_use_id, name, input)| {
                let user_question_callback = user_question_callback.clone();
                async move {
                    let result = self
                        .tools
                        .execute(
                            &name,
                            input.clone(),
                            ToolContext {
                                tool_use_id,
                                app_state: Some(app_state.clone()),
                                agent_context: self.agent_context.clone(),
                                user_question_callback: user_question_callback.clone(),
                            },
                        )
                        .await;
                    (index, name, input, result)
                }
            },
        ))
        .await;

        for (index, name, input, result) in concurrent_results {
            let result = result.map_err(|error| QueryLoopError::Tool(error.to_string()))?;
            if let Some(output) = &self.output {
                output.tool_result(&name, &result.content, result.is_error);
            }
            // Fire PostToolUse hooks
            if let Some(runner) = &self.hook_runner {
                let cwd = { app_state.lock().await.cwd.clone() };
                runner
                    .run_post_tool_use_with_cwd(
                        &name,
                        &input,
                        &result.content,
                        result.is_error,
                        &session_id,
                        &cwd,
                    )
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
                        user_question_callback: user_question_callback.clone(),
                    },
                )
                .await
                .map_err(|error| QueryLoopError::Tool(error.to_string()))?;

            if let Some(output) = &self.output {
                output.tool_result(&name, &result.content, result.is_error);
            }
            // Fire PostToolUse hooks (serial)
            if let Some(runner) = &self.hook_runner {
                let cwd = { app_state.lock().await.cwd.clone() };
                runner
                    .run_post_tool_use_with_cwd(
                        &name,
                        &input,
                        &result.content,
                        result.is_error,
                        &session_id,
                        &cwd,
                    )
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
    use rust_claude_core::hooks::{HookConfig, HookEventGroup};
    use rust_claude_core::message::{ContentBlock, Message};
    use rust_claude_tools::{
        AskUserQuestionRequest, AskUserQuestionResponse, AskUserQuestionTool, BashTool, EnterPlanModeTool, FileReadTool,
        MonitorTool, Tool, ToolContext, ToolError,
    };
    use std::collections::{HashMap, VecDeque};
    use std::path::PathBuf;
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
    async fn test_query_loop_ask_user_question_uses_ui_response() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "AskUserQuestion",
                        serde_json::json!({
                            "question": "Pick one",
                            "options": [
                                { "label": "A", "description": "Answer A" },
                                { "label": "B", "description": "Answer B" }
                            ],
                            "allow_custom": true
                        }),
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

        struct MockUserQuestionUI;

        #[async_trait]
        impl UserQuestionUI for MockUserQuestionUI {
            async fn ask(&self, _request: AskUserQuestionRequest) -> Option<AskUserQuestionResponse> {
                Some(AskUserQuestionResponse {
                    selected_label: Some("B".into()),
                    answer: "Answer B".into(),
                    custom: false,
                })
            }
        }

        let mut tools = ToolRegistry::new();
        tools.register(AskUserQuestionTool::new());
        let loop_runner = QueryLoop::new(client, tools)
            .with_user_question_ui(Box::new(MockUserQuestionUI));
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));

        let final_message = loop_runner.run(app_state.clone(), "choose").await.unwrap();

        assert_eq!(final_message.content, vec![ContentBlock::text("done")]);
        let state = app_state.lock().await;
        match &state.messages[2].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(!is_error);
                assert!(content.contains("\"selected_label\":\"B\""));
            }
            other => panic!("expected tool result, got {other:?}"),
        }
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
    async fn test_query_loop_hooks_use_updated_session_cwd_between_bash_calls() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let initial_dir = std::env::temp_dir().join(format!(
            "rust-claude-query-hook-initial-{}-{suffix}",
            std::process::id(),
        ));
        let target_dir = std::env::temp_dir().join(format!(
            "rust-claude-query-hook-target-{}-{suffix}",
            std::process::id(),
        ));
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();
        let hook_log = initial_dir.join("hook-log.jsonl");

        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Bash",
                        serde_json::json!({
                            "command": format!("cd {} && pwd", target_dir.display())
                        }),
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
                        serde_json::json!({ "command": "pwd" }),
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
        let mut hooks = HashMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![rust_claude_core::hooks::HookEventGroup {
                matcher: Some("Bash".to_string()),
                hooks: vec![rust_claude_core::hooks::HookConfig {
                    type_: "command".to_string(),
                    command: Some(format!(
                        "cat >> {}; printf '\\n' >> {}",
                        hook_log.display(),
                        hook_log.display()
                    )),
                    timeout: None,
                    once: false,
                }],
            }],
        );
        let hook_runner = Arc::new(HookRunner::new(hooks, initial_dir.clone()));
        let loop_runner = QueryLoop::new(client, tools)
            .with_max_rounds(4)
            .with_hook_runner(hook_runner);

        let mut state = AppState::new(initial_dir.clone());
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "multi").await.unwrap();

        assert_eq!(final_message.content, vec![ContentBlock::text("complete")]);
        assert_eq!(
            app_state.lock().await.cwd,
            target_dir.canonicalize().unwrap()
        );
        let log = tokio::fs::read_to_string(&hook_log).await.unwrap();
        let lines = log.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(&initial_dir.display().to_string()));
        assert!(lines[1].contains(&target_dir.display().to_string()));
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
    async fn test_query_loop_denies_monitor_command_before_spawn() {
        let marker =
            std::env::temp_dir().join(format!("rust-claude-monitor-denied-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let command = format!("printf started > {}", marker.display());

        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "Monitor",
                        serde_json::json!({
                            "command": command,
                            "pattern": "started",
                            "timeout": 1_000
                        }),
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
        tools.register(MonitorTool::new());
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::Default;
        state
            .always_deny_rules
            .push(rust_claude_core::permission::PermissionRule {
                tool_name: "Monitor".to_string(),
                pattern: Some(command),
                path_pattern: None,
                rule_type: rust_claude_core::permission::RuleType::Deny,
            });
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "monitor").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("done")]);
        assert!(!marker.exists());

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
    async fn test_query_loop_denies_mutating_tool_after_entering_plan_mode() {
        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![
                CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::tool_use(
                        "tool_1",
                        "EnterPlanMode",
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
                    content: vec![ContentBlock::tool_use(
                        "tool_2",
                        "SlowWrite",
                        serde_json::json!({}),
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
                    content: vec![ContentBlock::text("planned")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                },
            ])),
        };

        let mut tools = ToolRegistry::new();
        tools.register(EnterPlanModeTool::new());
        tools.register(SlowWriteTool);
        let loop_runner = QueryLoop::new(client, tools).with_max_rounds(4);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = rust_claude_core::permission::PermissionMode::BypassPermissions;
        let app_state = Arc::new(Mutex::new(state));

        let final_message = loop_runner.run(app_state.clone(), "plan").await.unwrap();
        assert_eq!(final_message.content, vec![ContentBlock::text("planned")]);

        let state = app_state.lock().await;
        assert_eq!(
            state.permission_mode,
            rust_claude_core::permission::PermissionMode::Plan
        );
        assert_eq!(state.messages.len(), 6);
        match &state.messages[4].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert!(*is_error);
                assert!(content.contains("Permission denied"));
                assert!(content.contains("Plan mode"));
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

    #[tokio::test]
    async fn test_query_loop_passes_real_session_id_to_hooks() {
        let path = std::env::temp_dir().join(format!(
            "rust-claude-query-hook-{}-{}.log",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));

        let client = MockClient {
            responses: Mutex::new(VecDeque::from(vec![CreateMessageResponse {
                id: "msg_1".to_string(),
                response_type: "message".to_string(),
                role: rust_claude_core::message::Role::Assistant,
                content: vec![ContentBlock::text("ok")],
                model: "claude-test".to_string(),
                stop_reason: Some(StopReason::EndTurn),
                stop_sequence: None,
                usage: usage(),
            }])),
        };
        let tools = ToolRegistry::new();
        let mut hooks = std::collections::HashMap::new();
        hooks.insert(
            "UserPromptSubmit".to_string(),
            vec![HookEventGroup {
                matcher: None,
                hooks: vec![HookConfig {
                    type_: "command".to_string(),
                    command: Some(format!("cat > {}", path.display())),
                    timeout: None,
                    once: false,
                }],
            }],
        );
        let runner = Arc::new(crate::hooks::HookRunner::new(hooks, PathBuf::from("/tmp")));
        let loop_runner = QueryLoop::new(client, tools).with_hook_runner(runner);

        let mut state = AppState::new(PathBuf::from("/tmp"));
        state.session.id = "session-xyz".into();
        let app_state = Arc::new(Mutex::new(state));

        let _ = loop_runner.run(app_state, "hello").await.unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"session_id\":\"session-xyz\""));
        let _ = std::fs::remove_file(path);
    }

    // --- Reactive compaction & model fallback tests ---

    /// A MockClient that can return errors for the first N calls, then successes.
    struct MockClientWithErrors {
        /// Sequence of results: Left(error) or Right(response)
        results: Mutex<VecDeque<Result<CreateMessageResponse, ApiError>>>,
    }

    #[async_trait]
    impl ModelClient for MockClientWithErrors {
        async fn create_message(
            &self,
            _request: &CreateMessageRequest,
        ) -> Result<CreateMessageResponse, ApiError> {
            let mut results = self.results.lock().await;
            results
                .pop_front()
                .expect("mock result should exist")
        }

        async fn create_message_stream(
            &self,
            request: &CreateMessageRequest,
        ) -> Result<MessageStream, ApiError> {
            // For simplicity, delegate to create_message
            let result = self.create_message(request).await?;
            let stop_reason = result.stop_reason.clone();
            let usage_val = result.usage.clone();
            let message_id = result.id.clone();
            let role = result.role.clone();
            let model = result.model.clone();
            let content = result.content.clone();

            let mut events = vec![Ok(StreamEvent::MessageStart {
                message: rust_claude_api::StreamMessage {
                    id: message_id,
                    role,
                    model,
                    content: vec![],
                    stop_reason: None,
                    stop_sequence: None,
                    usage: usage_val.clone(),
                },
            })];

            for (index, block) in content.into_iter().enumerate() {
                let delta = match &block {
                    ContentBlock::Text { text } => {
                        Some(rust_claude_api::ContentBlockDelta::TextDelta { text: text.clone() })
                    }
                    _ => None,
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
                usage: Some(usage_val),
            }));

            Ok(Box::pin(futures_util::stream::iter(events)))
        }
    }

    fn ok_response(text: &str) -> Result<CreateMessageResponse, ApiError> {
        Ok(CreateMessageResponse {
            id: "msg_ok".to_string(),
            response_type: "message".to_string(),
            role: rust_claude_core::message::Role::Assistant,
            content: vec![ContentBlock::text(text)],
            model: "claude-test".to_string(),
            stop_reason: Some(StopReason::EndTurn),
            stop_sequence: None,
            usage: usage(),
        })
    }

    #[tokio::test]
    async fn test_prompt_too_long_triggers_reactive_compaction() {
        // Mock responses:
        // 1. PromptTooLong (main call — simulates the API rejecting the prompt)
        // 2. Compaction summary response (consumed by force_compact LLM call)
        // 3. Retry main call (success — after compaction reduced context)
        let client = MockClientWithErrors {
            results: Mutex::new(VecDeque::from(vec![
                Err(ApiError::PromptTooLong("prompt is too long".to_string())),
                // Compaction summary response
                Ok(CreateMessageResponse {
                    id: "msg_compact".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("Summary of conversation")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                }),
                ok_response("recovered after compaction"),
            ])),
        };
        let tools = ToolRegistry::new();
        // Use a large context window so auto-compaction doesn't trigger after reactive compaction
        let compaction_config = rust_claude_core::compaction::CompactionConfig {
            context_window: 200_000,
            threshold_ratio: 0.8,
            preserve_ratio: 0.5,
            summary_max_tokens: 8192,
            ..Default::default()
        };
        let loop_runner = QueryLoop::new(client, tools)
            .with_compaction_config(compaction_config);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.stream = false;
        let app_state = Arc::new(Mutex::new(state));

        // Add enough messages for compaction to work (>4 messages required)
        {
            let mut s = app_state.lock().await;
            for i in 0..10 {
                if i % 2 == 0 {
                    s.add_message(Message::user("x".repeat(400)));
                } else {
                    s.add_message(Message::assistant(vec![ContentBlock::text(
                        "y".repeat(400),
                    )]));
                }
            }
        }

        let result = loop_runner.run(app_state, "test prompt").await.unwrap();
        assert_eq!(
            result.content,
            vec![ContentBlock::text("recovered after compaction")]
        );
    }

    #[tokio::test]
    async fn test_prompt_too_long_stage3_fails_after_exhaustion() {
        // Three PromptTooLong errors with no compaction config → should fail
        let client = MockClientWithErrors {
            results: Mutex::new(VecDeque::from(vec![
                Err(ApiError::PromptTooLong("too long 1".to_string())),
                Err(ApiError::PromptTooLong("too long 2".to_string())),
                Err(ApiError::PromptTooLong("too long 3".to_string())),
            ])),
        };
        let tools = ToolRegistry::new();
        // No compaction config = no compaction available
        let loop_runner = QueryLoop::new(client, tools);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.stream = false;
        let app_state = Arc::new(Mutex::new(state));

        let result = loop_runner.run(app_state, "test").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QueryLoopError::Api(ApiError::PromptTooLong(_))
        ));
    }

    #[tokio::test]
    async fn test_overloaded_backoff_and_retry() {
        // Two overloaded errors, then success
        let client = MockClientWithErrors {
            results: Mutex::new(VecDeque::from(vec![
                Err(ApiError::Overloaded("overloaded 1".to_string())),
                Err(ApiError::Overloaded("overloaded 2".to_string())),
                ok_response("recovered from overload"),
            ])),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools).with_max_rounds(10);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.stream = false;
        let app_state = Arc::new(Mutex::new(state));

        let start = Instant::now();
        let result = loop_runner.run(app_state, "test").await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(
            result.content,
            vec![ContentBlock::text("recovered from overload")]
        );
        // Should have waited at least 1s + 2s = 3s for backoff
        assert!(
            elapsed >= Duration::from_secs(3),
            "expected >= 3s backoff, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_overloaded_fallback_model_switch() {
        // 4 overloaded errors (exceeds MAX_OVERLOAD_RETRIES=3), then success with fallback model
        let client = MockClientWithErrors {
            results: Mutex::new(VecDeque::from(vec![
                Err(ApiError::Overloaded("overloaded".to_string())),
                Err(ApiError::Overloaded("overloaded".to_string())),
                Err(ApiError::Overloaded("overloaded".to_string())),
                Err(ApiError::Overloaded("overloaded".to_string())),
                ok_response("fallback model response"),
            ])),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools).with_max_rounds(10);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.stream = false;
        state.config.fallback_model = Some("claude-haiku".to_string());
        let app_state = Arc::new(Mutex::new(state));

        let result = loop_runner.run(app_state.clone(), "test").await.unwrap();
        assert_eq!(
            result.content,
            vec![ContentBlock::text("fallback model response")]
        );

        // Verify model was switched
        let s = app_state.lock().await;
        assert_eq!(s.session.model_setting, "claude-haiku");
        assert_eq!(s.session.model, "claude-haiku");
    }

    #[tokio::test]
    async fn test_overloaded_no_fallback_continues_retrying() {
        // 4 overloaded errors with no fallback configured, then success
        let client = MockClientWithErrors {
            results: Mutex::new(VecDeque::from(vec![
                Err(ApiError::Overloaded("overloaded".to_string())),
                Err(ApiError::Overloaded("overloaded".to_string())),
                Err(ApiError::Overloaded("overloaded".to_string())),
                Err(ApiError::Overloaded("overloaded".to_string())),
                ok_response("eventually recovered"),
            ])),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools).with_max_rounds(10);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.stream = false;
        // No fallback_model configured
        let app_state = Arc::new(Mutex::new(state));

        let result = loop_runner.run(app_state, "test").await.unwrap();
        assert_eq!(
            result.content,
            vec![ContentBlock::text("eventually recovered")]
        );
    }

    #[tokio::test]
    async fn test_overloaded_counter_resets_on_success() {
        // Overloaded, then success, then overloaded again — counter should have reset
        let client = MockClientWithErrors {
            results: Mutex::new(VecDeque::from(vec![
                Err(ApiError::Overloaded("overloaded".to_string())),
                // This response has tool_use to continue the loop
                Ok(CreateMessageResponse {
                    id: "msg_1".to_string(),
                    response_type: "message".to_string(),
                    role: rust_claude_core::message::Role::Assistant,
                    content: vec![ContentBlock::text("middle")],
                    model: "claude-test".to_string(),
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                    usage: usage(),
                }),
            ])),
        };
        let tools = ToolRegistry::new();
        let loop_runner = QueryLoop::new(client, tools).with_max_rounds(10);

        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.session.stream = false;
        let app_state = Arc::new(Mutex::new(state));

        // Should succeed — overload count resets after the successful response
        let result = loop_runner.run(app_state, "test").await.unwrap();
        assert_eq!(result.content, vec![ContentBlock::text("middle")]);
    }
}
