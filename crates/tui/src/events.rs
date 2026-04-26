use crossterm::event::{KeyEvent, MouseEvent};
use rust_claude_core::compaction::CompactionResult;
use rust_claude_core::config::{ConfigProvenance, Theme};
use rust_claude_core::state::TodoItem;

use crate::diff::DiffLine;

/// Commands emitted by the TUI input layer toward the background worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserCommand {
    Prompt(String),
    Compact,
    SetMode(String),
    SetModel(String),
    SetTheme(Theme),
    CancelStream,
    ShowConfig,
    ShowCost,
    ShowDiff,
    ShowHooks,
    ShowMemory,
    RememberMemory {
        memory_type: String,
        path: String,
        title: String,
        description: String,
        body: String,
    },
    ForgetMemory {
        path: String,
    },
    ShowMcp,
}

/// Events consumed by the TUI application.
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input from the terminal.
    Key(KeyEvent),
    /// Mouse input from the terminal.
    Mouse(MouseEvent),
    /// Bracketed paste input from the terminal.
    Paste(String),
    /// A new assistant streaming response has started.
    StreamStart,
    /// A chunk of streaming text from the assistant.
    StreamDelta(String),
    /// The assistant finished streaming its response.
    StreamEnd,
    /// The active stream was cancelled by the user.
    StreamCancelled,
    /// The model is thinking (extended thinking / reasoning).
    ThinkingStart,
    /// A chunk of streaming thinking text.
    ThinkingDelta(String),
    /// A completed thinking block (used by non-streaming flow).
    ThinkingComplete(String),
    /// The model began constructing a tool call (streaming).
    ToolInputStreamStart {
        name: String,
    },
    /// A chunk of tool input JSON during streaming construction.
    ToolInputDelta {
        name: String,
        json_fragment: String,
    },
    /// The model began a tool call (complete input available).
    ToolUseStart {
        name: String,
        input: serde_json::Value,
    },
    /// A tool has returned a result.
    ToolResult {
        name: String,
        output: String,
        is_error: bool,
    },
    /// Complete assistant text message (non-streaming fallback).
    AssistantMessage(String),
    /// Token usage update.
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    },
    /// Update status bar info.
    StatusUpdate {
        model: String,
        model_setting: String,
        permission_mode: String,
        git_branch: Option<String>,
    },
    ConfigInfo {
        model_source: String,
        permission_source: String,
        base_url_source: String,
        theme_source: String,
    },
    /// An error to display to the user.
    Error(String),
    /// Terminal resize event.
    Resize(u16, u16),
    /// A tool needs permission confirmation from the user.
    PermissionRequest {
        tool_name: String,
        input: serde_json::Value,
        /// Channel to send the user's response back.
        response_tx: tokio::sync::oneshot::Sender<PermissionResponse>,
    },
    /// Task list has been updated.
    TodoUpdate(Vec<TodoItem>),
    /// Conversation compaction has started.
    CompactionStart,
    /// Conversation compaction completed successfully.
    CompactionComplete {
        result: CompactionResult,
    },
    /// A PreToolUse hook blocked a tool.
    HookBlocked {
        tool_name: String,
        reason: String,
    },
}

/// The user's response to a permission confirmation dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionResponse {
    /// Allow this one invocation.
    Allow,
    /// Allow and add an always-allow rule.
    AlwaysAllow,
    /// Deny this one invocation.
    Deny,
    /// Deny and add an always-deny rule.
    AlwaysDeny,
}

/// Messages that the TUI's chat area displays.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Assistant(String),
    Thinking {
        summary: String,
        content: String,
    },
    ToolUse {
        name: String,
        input_summary: String,
        /// Computed diff lines for FileEdit/FileWrite (None for other tools).
        diff_lines: Option<Vec<DiffLine>>,
    },
    ToolResult {
        name: String,
        output_summary: String,
        is_error: bool,
    },
    System(String),
}

impl ChatMessage {
    /// Produce a one-line prefix label for rendering.
    pub fn prefix(&self) -> &'static str {
        match self {
            ChatMessage::User(_) => "You: ",
            ChatMessage::Assistant(_) => "Claude: ",
            ChatMessage::Thinking { .. } => "Thinking: ",
            ChatMessage::ToolUse { .. } => "Tool: ",
            ChatMessage::ToolResult { is_error: true, .. } => "Error: ",
            ChatMessage::ToolResult { .. } => "Result: ",
            ChatMessage::System(_) => "System: ",
        }
    }

    /// The body text of the message.
    pub fn body(&self) -> &str {
        match self {
            ChatMessage::User(s)
            | ChatMessage::Assistant(s)
            | ChatMessage::System(s) => s,
            ChatMessage::Thinking { content, .. } => content,
            ChatMessage::ToolUse { name, input_summary, .. } => {
                if input_summary.is_empty() {
                    name
                } else {
                    input_summary
                }
            }
            ChatMessage::ToolResult { output_summary, .. } => output_summary,
        }
    }

    /// Map internal tool names to user-facing display names matching Claude Code.
    pub fn user_facing_tool_name(tool_name: &str) -> &str {
        match tool_name {
            "Bash" => "Bash",
            "FileRead" => "Read",
            "FileEdit" => "Update",
            "FileWrite" => "Write",
            "Task" => "Task",
            "TodoWrite" => "Task",
            "Lsp" => "LSP",
            "WebFetch" => "Fetch",
            "WebSearch" => "Search",
            other => other,
        }
    }
}

pub fn format_provenance_summary(provenance: &ConfigProvenance) -> (String, String, String, String) {
    (
        provenance.model.to_string(),
        format!("allow:{} deny:{}", provenance.always_allow, provenance.always_deny),
        provenance.base_url.to_string(),
        provenance.theme.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_prefix() {
        assert_eq!(ChatMessage::User("hi".into()).prefix(), "You: ");
        assert_eq!(ChatMessage::Assistant("hello".into()).prefix(), "Claude: ");
        assert_eq!(
            ChatMessage::Thinking {
                summary: "Thought for ~10 tokens".into(),
                content: "reasoning".into()
            }
            .prefix(),
            "Thinking: "
        );
        assert_eq!(
            ChatMessage::ToolUse {
                name: "Bash".into(),
                input_summary: "ls".into(),
                diff_lines: None,
            }
            .prefix(),
            "Tool: "
        );
        assert_eq!(
            ChatMessage::ToolResult {
                name: "Bash".into(),
                output_summary: "ok".into(),
                is_error: false
            }
            .prefix(),
            "Result: "
        );
        assert_eq!(
            ChatMessage::ToolResult {
                name: "Bash".into(),
                output_summary: "fail".into(),
                is_error: true
            }
            .prefix(),
            "Error: "
        );
        assert_eq!(ChatMessage::System("info".into()).prefix(), "System: ");
    }
}
