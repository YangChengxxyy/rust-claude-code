use std::sync::Arc;

use rust_claude_core::compaction::needs_compaction;

use rust_claude_api::{ApiMessage, CreateMessageRequest, SystemBlock, SystemPrompt};
use rust_claude_core::{
    claude_md,
    compaction::{
        estimate_current_tokens, estimate_message_tokens, estimate_system_prompt_tokens,
        partition_messages, CompactionConfig, CompactionResult,
    },
    message::{ContentBlock, Message, Role},
    model::{
        get_runtime_main_loop_model, get_thinking_config_for_model, normalize_model_string_for_api,
    },
    state::{AppState, McpToolUsage, PermissionDecisionRecord},
};
use tokio::sync::Mutex;

use rust_claude_api::ModelClient;

const COMPACTION_PROMPT: &str = r#"You are a conversation summarizer. Your task is to create a concise but comprehensive summary of the conversation history provided below.

IMPORTANT: Your summary MUST preserve the following:
1. All file paths mentioned (exact paths, not paraphrased)
2. All tool calls and their outcomes (tool name, key inputs, whether they succeeded or failed)
3. Key decisions made during the conversation
4. The overall conversation flow and context
5. Any errors encountered and how they were resolved
6. Current state of any ongoing work

Format the summary as a clear, structured narrative. Use bullet points for lists of files or tool calls. Do NOT include meta-commentary about the summarization process itself.

The summary will replace the original messages in the conversation, so it must contain enough context for the conversation to continue naturally."#;

const MICRO_COMPACT_PLACEHOLDER: &str = "[Content cleared to reduce context size]";
const MICRO_COMPACT_PRESERVE_TURNS: usize = 3;

/// Result of a micro-compaction operation (no LLM call).
#[derive(Debug, Clone)]
pub struct MicroCompactionResult {
    pub blocks_cleared: usize,
    pub estimated_token_reduction: u32,
}

#[derive(Debug, Clone, Default)]
struct CompactionContext {
    project_guidance: Option<String>,
    used_mcp_tools: Vec<McpToolUsage>,
    permission_decisions: Vec<PermissionDecisionRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompactionError {
    #[error("API error during summary generation: {0}")]
    Api(#[from] rust_claude_api::ApiError),
    #[error("compaction not needed")]
    NotNeeded,
    #[error("too few messages to compact")]
    TooFewMessages,
    #[error("summary generation returned no text")]
    EmptySummary,
}

pub struct CompactionService<C> {
    client: C,
    config: CompactionConfig,
}

impl<C: ModelClient> CompactionService<C> {
    pub fn new(client: C, config: CompactionConfig) -> Self {
        Self { client, config }
    }

    /// Generate a summary of the given messages by calling the LLM.
    async fn generate_summary(
        &self,
        messages_to_compact: &[Message],
        model: &str,
        context: &CompactionContext,
    ) -> Result<String, CompactionError> {
        // Format the messages into a readable transcript for summarization
        let mut transcript = String::new();
        for msg in messages_to_compact {
            let role_label = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        transcript.push_str(&format!("[{role_label}]: {text}\n\n"));
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        transcript.push_str(&format!(
                            "[{role_label} - Tool Call: {name}]: {}\n\n",
                            serde_json::to_string(input).unwrap_or_default()
                        ));
                    }
                    ContentBlock::ToolResult {
                        content, is_error, ..
                    } => {
                        let label = if *is_error { "Error" } else { "Result" };
                        transcript.push_str(&format!("[Tool {label}]: {content}\n\n"));
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        transcript.push_str(&format!("[Thinking]: {thinking}\n\n"));
                    }
                    ContentBlock::Image { source } => {
                        let description = match source {
                            rust_claude_core::message::ImageSource::Base64 {
                                media_type, ..
                            } => {
                                format!("embedded {media_type} image")
                            }
                            rust_claude_core::message::ImageSource::Url { url } => {
                                format!("image at {url}")
                            }
                        };
                        transcript.push_str(&format!("[{role_label} - Image]: {description}\n\n"));
                    }
                    ContentBlock::Unknown => {}
                }
            }
        }

        let context_text = self.format_context(context);
        let user_message = ApiMessage::from(&Message::user(if context_text.is_empty() {
            format!("Please summarize the following conversation:\n\n{transcript}")
        } else {
            format!(
                "Please summarize the following conversation. Preserve the supplemental context in the resulting summary where relevant.\n\n{context_text}\n\nConversation:\n\n{transcript}"
            )
        }));

        let mut request =
            CreateMessageRequest::new(normalize_model_string_for_api(model), vec![user_message])
                .with_max_tokens(self.config.summary_max_tokens);

        // Use structured system prompt with cache_control for compaction requests
        request.system = Some(SystemPrompt::StructuredBlocks(vec![SystemBlock::text(
            COMPACTION_PROMPT,
        )
        .with_cache_control()]));

        // Enable thinking for compaction summary generation on supported models
        let thinking_config = get_thinking_config_for_model(model, true, None);
        if let Some(thinking_value) = thinking_config.to_api_value(self.config.summary_max_tokens) {
            request = request.with_thinking(thinking_value);
        }

        let response = self.client.create_message(&request).await?;

        let summary = response
            .content
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if summary.is_empty() {
            return Err(CompactionError::EmptySummary);
        }

        Ok(summary)
    }

    fn collect_context(&self, state: &AppState) -> CompactionContext {
        let project_guidance = self.collect_project_guidance(&state.cwd);
        let used_mcp_tools = state
            .used_mcp_tools
            .iter()
            .rev()
            .take(self.config.mcp_tool_limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let permission_decisions = state
            .recent_permission_decisions
            .iter()
            .rev()
            .take(self.config.permission_decision_limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        CompactionContext {
            project_guidance,
            used_mcp_tools,
            permission_decisions,
        }
    }

    fn collect_project_guidance(&self, cwd: &std::path::Path) -> Option<String> {
        let mut guidance = claude_md::discover_claude_md(cwd)
            .into_iter()
            .filter(|file| {
                matches!(
                    file.source_type,
                    claude_md::ClaudeMdSourceType::Project
                        | claude_md::ClaudeMdSourceType::ProjectLocal
                        | claude_md::ClaudeMdSourceType::Rule
                )
            })
            .map(|file| format!("## {}\n{}", file.path.display(), file.content.trim()))
            .filter(|section| !section.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        if guidance.trim().is_empty() {
            return None;
        }

        if guidance.len() > self.config.project_guidance_char_limit {
            let mut end = self.config.project_guidance_char_limit;
            while end > 0 && !guidance.is_char_boundary(end) {
                end -= 1;
            }
            guidance.truncate(end);
            guidance.push_str("\n... project guidance truncated ...");
        }

        Some(guidance)
    }

    fn format_context(&self, context: &CompactionContext) -> String {
        let mut sections = Vec::new();
        if let Some(project_guidance) = &context.project_guidance {
            sections.push(format!("Project guidance:\n{project_guidance}"));
        }
        if !context.used_mcp_tools.is_empty() {
            let tools = context
                .used_mcp_tools
                .iter()
                .map(|usage| format!("- {} / {}", usage.server_name, usage.tool_name))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("Used MCP tools:\n{tools}"));
        }
        if !context.permission_decisions.is_empty() {
            let decisions = context
                .permission_decisions
                .iter()
                .map(|decision| match &decision.command {
                    Some(command) => format!(
                        "- {}: {} ({})",
                        decision.tool_name, decision.decision, command
                    ),
                    None => format!("- {}: {}", decision.tool_name, decision.decision),
                })
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("Recent permission decisions:\n{decisions}"));
        }

        sections.join("\n\n")
    }

    /// Perform compaction on the conversation in AppState.
    ///
    /// Returns `Ok(result)` if compaction was performed, or an error if
    /// compaction was not needed or failed.
    pub async fn compact(
        &self,
        app_state: &Arc<Mutex<AppState>>,
    ) -> Result<CompactionResult, CompactionError> {
        let (
            messages,
            system_prompt,
            permission_mode,
            model_setting,
            last_api_usage,
            last_api_message_index,
            context,
        ) = {
            let state = app_state.lock().await;
            (
                state.messages.clone(),
                state.session.system_prompt.clone(),
                state.permission_mode,
                state.session.model_setting.clone(),
                state.last_api_usage.clone(),
                state.last_api_message_index,
                self.collect_context(&state),
            )
        };

        if messages.len() <= 4 {
            return Err(CompactionError::TooFewMessages);
        }

        let estimated_tokens_before = estimate_current_tokens(
            &system_prompt,
            &messages,
            last_api_usage.as_ref(),
            last_api_message_index,
        );

        let (to_compact, to_preserve) = partition_messages(&self.config, &messages);

        if to_compact.is_empty() {
            return Err(CompactionError::NotNeeded);
        }

        let original_count = messages.len();
        let compacted_count = to_compact.len();
        let preserved_count = to_preserve.len();

        let runtime_model = get_runtime_main_loop_model(&model_setting, permission_mode, false);
        let summary = self
            .generate_summary(&to_compact, &runtime_model, &context)
            .await?;
        let context_text = self.format_context(&context);
        let compacted_text = if context_text.is_empty() {
            format!("[COMPACTED]\n\n{summary}")
        } else {
            format!("[COMPACTED]\n\n{summary}\n\n[COMPACTION CONTEXT]\n\n{context_text}")
        };
        let summary_length = compacted_text.len();

        let summary_message = Message::user(compacted_text);

        let mut new_messages = Vec::with_capacity(1 + preserved_count);
        new_messages.push(summary_message);
        new_messages.extend(to_preserve);

        let estimated_tokens_after =
            estimate_system_prompt_tokens(&system_prompt) + estimate_message_tokens(&new_messages);

        {
            let mut state = app_state.lock().await;
            state.messages = new_messages;
            // Reset stale usage tracking since the message indices have changed.
            state.last_api_usage = None;
            state.last_api_message_index = 0;
        }

        Ok(CompactionResult {
            original_message_count: original_count,
            compacted_message_count: compacted_count,
            preserved_message_count: preserved_count,
            estimated_tokens_before,
            estimated_tokens_after,
            summary_length,
        })
    }

    /// Force compaction regardless of threshold (for /compact command).
    pub async fn force_compact(
        &self,
        app_state: &Arc<Mutex<AppState>>,
    ) -> Result<CompactionResult, CompactionError> {
        let messages_len = {
            let state = app_state.lock().await;
            state.messages.len()
        };

        if messages_len <= 4 {
            return Err(CompactionError::TooFewMessages);
        }

        self.compact(app_state).await
    }

    /// Perform micro-compaction: replace old ToolResult content with a placeholder.
    pub async fn micro_compact(
        &self,
        app_state: &Arc<Mutex<AppState>>,
    ) -> Result<MicroCompactionResult, CompactionError> {
        self.micro_compact_with_window(app_state, MICRO_COMPACT_PRESERVE_TURNS).await
    }

    /// Micro-compaction with a custom preservation window.
    pub async fn micro_compact_with_window(
        &self,
        app_state: &Arc<Mutex<AppState>>,
        preserve_turns: usize,
    ) -> Result<MicroCompactionResult, CompactionError> {
        let mut state = app_state.lock().await;
        let msg_count = state.messages.len();

        if msg_count == 0 {
            return Ok(MicroCompactionResult {
                blocks_cleared: 0,
                estimated_token_reduction: 0,
            });
        }

        let mut assistant_count = 0;
        let mut preserve_from_index = 0;
        for i in (0..msg_count).rev() {
            if state.messages[i].role == Role::Assistant {
                assistant_count += 1;
                if assistant_count >= preserve_turns {
                    preserve_from_index = i;
                    break;
                }
            }
        }

        let mut blocks_cleared: usize = 0;
        let mut estimated_token_reduction: u32 = 0;

        for i in 0..preserve_from_index {
            let msg = &mut state.messages[i];
            for block in &mut msg.content {
                if let ContentBlock::ToolResult {
                    content, is_error, ..
                } = block
                {
                    if content.is_empty()
                        || *content == MICRO_COMPACT_PLACEHOLDER
                        || *is_error
                    {
                        continue;
                    }

                    let old_tokens = (content.len() as u32) / 4;
                    let placeholder_tokens = (MICRO_COMPACT_PLACEHOLDER.len() as u32) / 4;
                    if old_tokens > placeholder_tokens {
                        estimated_token_reduction += old_tokens - placeholder_tokens;
                    }

                    *content = MICRO_COMPACT_PLACEHOLDER.to_string();
                    blocks_cleared += 1;
                }
            }
        }

        Ok(MicroCompactionResult {
            blocks_cleared,
            estimated_token_reduction,
        })
    }

    /// Check if compaction is needed and perform it if so.
    pub async fn compact_if_needed(
        &self,
        app_state: &Arc<Mutex<AppState>>,
    ) -> Result<Option<CompactionResult>, CompactionError> {
        let (messages, system_prompt, last_api_usage, last_api_message_index) = {
            let state = app_state.lock().await;
            (
                state.messages.clone(),
                state.session.system_prompt.clone(),
                state.last_api_usage.clone(),
                state.last_api_message_index,
            )
        };

        if !needs_compaction(
            &self.config,
            &system_prompt,
            &messages,
            last_api_usage.as_ref(),
            last_api_message_index,
        ) {
            return Ok(None);
        }

        self.compact(app_state).await.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rust_claude_api::{ApiError, CreateMessageResponse, MessageStream};
    use rust_claude_core::message::Usage;
    use rust_claude_core::state::{McpToolUsage, PermissionDecisionRecord};
    use std::collections::VecDeque;
    use std::fs;

    struct MockCompactionClient {
        summary_text: String,
        should_fail: bool,
    }

    #[async_trait]
    impl ModelClient for MockCompactionClient {
        async fn create_message(
            &self,
            _request: &CreateMessageRequest,
        ) -> Result<CreateMessageResponse, ApiError> {
            if self.should_fail {
                return Err(ApiError::Auth("mock failure".to_string()));
            }
            Ok(CreateMessageResponse {
                id: "msg_test".to_string(),
                response_type: "message".to_string(),
                role: Role::Assistant,
                content: vec![ContentBlock::text(self.summary_text.clone())],
                model: "test-model".to_string(),
                stop_reason: None,
                stop_sequence: None,
                usage: Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
        }

        async fn create_message_stream(
            &self,
            _request: &CreateMessageRequest,
        ) -> Result<MessageStream, ApiError> {
            unimplemented!("stream not used in compaction tests")
        }
    }

    fn make_messages(count: usize, text_size: usize) -> Vec<Message> {
        let text = "x".repeat(text_size);
        (0..count)
            .map(|i| {
                if i % 2 == 0 {
                    Message::user(text.clone())
                } else {
                    Message::assistant(vec![ContentBlock::text(text.clone())])
                }
            })
            .collect()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rust-claude-compaction-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn compact_test_config() -> CompactionConfig {
        CompactionConfig {
            context_window: 1000,
            threshold_ratio: 0.8,
            preserve_ratio: 0.5,
            summary_max_tokens: 8192,
            project_guidance_char_limit: 200,
            mcp_tool_limit: 2,
            permission_decision_limit: 2,
        }
    }

    #[tokio::test]
    async fn test_compact_success() {
        let client = MockCompactionClient {
            summary_text: "This is a summary of the conversation.".to_string(),
            should_fail: false,
        };
        let config = CompactionConfig {
            context_window: 1000,
            threshold_ratio: 0.8,
            preserve_ratio: 0.5,
            summary_max_tokens: 8192,
            ..Default::default()
        };
        let service = CompactionService::new(client, config);

        let state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut s = state.lock().await;
            // Add enough messages to trigger compaction
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        let result = service.compact(&state).await.unwrap();
        assert!(result.compacted_message_count > 0);
        assert!(result.preserved_message_count > 0);
        assert_eq!(
            result.compacted_message_count + result.preserved_message_count,
            result.original_message_count
        );

        // Verify the state was updated
        let s = state.lock().await;
        assert!(s.messages.len() < result.original_message_count);
        // First message should be the compacted summary
        if let ContentBlock::Text { text } = &s.messages[0].content[0] {
            assert!(text.starts_with("[COMPACTED]"));
        } else {
            panic!("first message should be text");
        }
    }

    #[tokio::test]
    async fn test_compact_too_few_messages() {
        let client = MockCompactionClient {
            summary_text: "summary".to_string(),
            should_fail: false,
        };
        let service = CompactionService::new(client, CompactionConfig::default());

        let state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut s = state.lock().await;
            s.add_message(Message::user("hello"));
            s.add_message(Message::assistant(vec![ContentBlock::text("hi")]));
        }

        let result = service.compact(&state).await;
        assert!(matches!(result, Err(CompactionError::TooFewMessages)));
    }

    #[tokio::test]
    async fn test_compact_api_failure() {
        let client = MockCompactionClient {
            summary_text: String::new(),
            should_fail: true,
        };
        let config = CompactionConfig {
            context_window: 1000,
            ..Default::default()
        };
        let service = CompactionService::new(client, config);

        let state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut s = state.lock().await;
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        let original_count = state.lock().await.messages.len();
        let result = service.compact(&state).await;
        assert!(result.is_err());
        // Messages should be unchanged after failure
        assert_eq!(state.lock().await.messages.len(), original_count);
    }

    #[tokio::test]
    async fn test_compact_if_needed_below_threshold() {
        let client = MockCompactionClient {
            summary_text: "summary".to_string(),
            should_fail: false,
        };
        let service = CompactionService::new(client, CompactionConfig::default());

        let state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut s = state.lock().await;
            for msg in make_messages(6, 100) {
                s.add_message(msg);
            }
        }

        let result = service.compact_if_needed(&state).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_compact_if_needed_above_threshold() {
        let client = MockCompactionClient {
            summary_text: "This summarizes the conversation.".to_string(),
            should_fail: false,
        };
        let config = CompactionConfig {
            context_window: 1000,
            threshold_ratio: 0.8,
            preserve_ratio: 0.5,
            summary_max_tokens: 8192,
            ..Default::default()
        };
        let service = CompactionService::new(client, config);

        let state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut s = state.lock().await;
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        let result = service.compact_if_needed(&state).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_compact_includes_project_guidance_when_available() {
        let dir = temp_dir("guidance");
        fs::write(dir.join("CLAUDE.md"), "# Rules\nUse careful Rust.").unwrap();
        let service = CompactionService::new(
            MockCompactionClient {
                summary_text: "summary".to_string(),
                should_fail: false,
            },
            compact_test_config(),
        );
        let state = Arc::new(Mutex::new(AppState::new(dir.clone())));
        {
            let mut s = state.lock().await;
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        service.compact(&state).await.unwrap();
        let s = state.lock().await;
        if let ContentBlock::Text { text } = &s.messages[0].content[0] {
            assert!(text.contains("Project guidance"));
            assert!(text.contains("Use careful Rust"));
        } else {
            panic!("first message should be text");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_compact_omits_project_guidance_when_unavailable() {
        let dir = temp_dir("no-guidance");
        let service = CompactionService::new(
            MockCompactionClient {
                summary_text: "summary".to_string(),
                should_fail: false,
            },
            compact_test_config(),
        );
        let state = Arc::new(Mutex::new(AppState::new(dir.clone())));
        {
            let mut s = state.lock().await;
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        service.compact(&state).await.unwrap();
        let s = state.lock().await;
        if let ContentBlock::Text { text } = &s.messages[0].content[0] {
            assert!(!text.contains("Project guidance"));
        } else {
            panic!("first message should be text");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_compact_includes_mcp_usage_when_available() {
        let dir = temp_dir("mcp");
        let service = CompactionService::new(
            MockCompactionClient {
                summary_text: "summary".to_string(),
                should_fail: false,
            },
            compact_test_config(),
        );
        let state = Arc::new(Mutex::new(AppState::new(dir.clone())));
        {
            let mut s = state.lock().await;
            s.used_mcp_tools.push(McpToolUsage {
                server_name: "filesystem".to_string(),
                tool_name: "read_file".to_string(),
            });
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        service.compact(&state).await.unwrap();
        let s = state.lock().await;
        if let ContentBlock::Text { text } = &s.messages[0].content[0] {
            assert!(text.contains("Used MCP tools"));
            assert!(text.contains("filesystem / read_file"));
        } else {
            panic!("first message should be text");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_compact_omits_mcp_usage_when_unavailable() {
        let dir = temp_dir("no-mcp");
        let service = CompactionService::new(
            MockCompactionClient {
                summary_text: "summary".to_string(),
                should_fail: false,
            },
            compact_test_config(),
        );
        let state = Arc::new(Mutex::new(AppState::new(dir.clone())));
        {
            let mut s = state.lock().await;
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        service.compact(&state).await.unwrap();
        let s = state.lock().await;
        if let ContentBlock::Text { text } = &s.messages[0].content[0] {
            assert!(!text.contains("Used MCP tools"));
        } else {
            panic!("first message should be text");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_compact_bounds_recent_permission_decisions() {
        let dir = temp_dir("permissions");
        let service = CompactionService::new(
            MockCompactionClient {
                summary_text: "summary".to_string(),
                should_fail: false,
            },
            compact_test_config(),
        );
        let state = Arc::new(Mutex::new(AppState::new(dir.clone())));
        {
            let mut s = state.lock().await;
            s.recent_permission_decisions = VecDeque::from(vec![
                PermissionDecisionRecord {
                    tool_name: "Bash".to_string(),
                    decision: "allowed".to_string(),
                    command: Some("one".to_string()),
                },
                PermissionDecisionRecord {
                    tool_name: "Monitor".to_string(),
                    decision: "denied".to_string(),
                    command: Some("two".to_string()),
                },
                PermissionDecisionRecord {
                    tool_name: "FileWrite".to_string(),
                    decision: "ask_unavailable".to_string(),
                    command: None,
                },
            ]);
            for msg in make_messages(10, 400) {
                s.add_message(msg);
            }
        }

        service.compact(&state).await.unwrap();
        let s = state.lock().await;
        if let ContentBlock::Text { text } = &s.messages[0].content[0] {
            assert!(text.contains("Recent permission decisions"));
            assert!(!text.contains("one"));
            assert!(text.contains("Monitor: denied (two)"));
            assert!(text.contains("FileWrite: ask_unavailable"));
        } else {
            panic!("first message should be text");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
