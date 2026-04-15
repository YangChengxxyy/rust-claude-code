use crossterm::event::KeyEvent;

/// Events consumed by the TUI application.
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input from the terminal.
    Key(KeyEvent),
    /// A chunk of streaming text from the assistant.
    StreamDelta(String),
    /// The assistant finished streaming its response.
    StreamEnd,
    /// The model began a tool call.
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
    },
    /// An error to display to the user.
    Error(String),
    /// Terminal resize event.
    Resize(u16, u16),
}

/// Messages that the TUI's chat area displays.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Assistant(String),
    ToolUse {
        name: String,
        input_summary: String,
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
            ChatMessage::ToolUse { name, input_summary } => {
                if input_summary.is_empty() {
                    name
                } else {
                    input_summary
                }
            }
            ChatMessage::ToolResult { output_summary, .. } => output_summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_prefix() {
        assert_eq!(ChatMessage::User("hi".into()).prefix(), "You: ");
        assert_eq!(ChatMessage::Assistant("hello".into()).prefix(), "Claude: ");
        assert_eq!(
            ChatMessage::ToolUse {
                name: "Bash".into(),
                input_summary: "ls".into()
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

    #[test]
    fn test_chat_message_body() {
        assert_eq!(ChatMessage::User("hello".into()).body(), "hello");
        assert_eq!(
            ChatMessage::ToolUse {
                name: "Bash".into(),
                input_summary: "".into()
            }
            .body(),
            "Bash"
        );
        assert_eq!(
            ChatMessage::ToolUse {
                name: "Bash".into(),
                input_summary: "echo hi".into()
            }
            .body(),
            "echo hi"
        );
    }
}
