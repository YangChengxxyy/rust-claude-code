use rust_claude_core::compaction::CompactionResult;
use rust_claude_core::state::TodoItem;
use rust_claude_tools::{AskUserQuestionRequest, AskUserQuestionResponse};

/// Decision returned by PermissionUI when a tool requires interactive confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    AllowAlways,
    Deny,
    DenyAlways,
}

/// Sink for streaming output events from the agent loop.
///
/// Implementations can forward these to a terminal UI, log them,
/// or discard them in headless mode.
pub trait OutputSink: Send + Sync {
    fn stream_start(&self) {}
    fn stream_delta(&self, _text: &str) {}
    fn stream_end(&self) {}
    fn stream_cancelled(&self) {}

    fn thinking_start(&self) {}
    fn thinking_delta(&self, _text: &str) {}
    fn thinking_complete(&self, _text: &str) {}

    fn tool_input_start(&self, _name: &str) {}
    fn tool_input_delta(&self, _name: &str, _json_fragment: &str) {}

    fn tool_use(&self, _name: &str, _input: &serde_json::Value) {}
    fn tool_result(&self, _name: &str, _output: &str, _is_error: bool) {}

    fn usage(
        &self,
        _input_tokens: u64,
        _output_tokens: u64,
        _cache_read_input_tokens: u64,
        _cache_creation_input_tokens: u64,
    ) {
    }

    fn error(&self, _message: &str) {}

    fn compaction_start(&self) {}
    fn compaction_complete(&self, _result: &CompactionResult) {}

    fn hook_blocked(&self, _tool_name: &str, _reason: &str) {}

    fn todo_update(&self, _todos: &[TodoItem]) {}
}

/// UI for interactive permission confirmation.
#[async_trait::async_trait]
pub trait PermissionUI: Send + Sync {
    /// Request the user's decision for a tool invocation.
    ///
    /// Returns `None` if the UI is unavailable (e.g., headless mode).
    async fn request(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Option<PermissionDecision>;
}

/// UI for structured user questions (AskUserQuestionTool).
#[async_trait::async_trait]
pub trait UserQuestionUI: Send + Sync {
    /// Ask the user a structured question and await their response.
    ///
    /// Returns `None` if the UI is unavailable.
    async fn ask(
        &self,
        request: AskUserQuestionRequest,
    ) -> Option<AskUserQuestionResponse>;
}

// No-op implementations for headless mode

/// An OutputSink that discards all streaming events.
pub struct NoopOutputSink;

impl OutputSink for NoopOutputSink {}

/// A PermissionUI that always denies (headless mode).
pub struct DenyAllPermissionUI;

#[async_trait::async_trait]
impl PermissionUI for DenyAllPermissionUI {
    async fn request(
        &self,
        _tool_name: &str,
        _input: &serde_json::Value,
    ) -> Option<PermissionDecision> {
        Some(PermissionDecision::Deny)
    }
}

/// A UserQuestionUI that always returns None (headless mode).
pub struct NoopUserQuestionUI;

#[async_trait::async_trait]
impl UserQuestionUI for NoopUserQuestionUI {
    async fn ask(
        &self,
        _request: AskUserQuestionRequest,
    ) -> Option<AskUserQuestionResponse> {
        None
    }
}
