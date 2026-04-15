use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::events::{AppEvent, ChatMessage};
use crate::ui;

/// Main TUI application state.
pub struct App {
    /// Chat message history.
    pub messages: Vec<ChatMessage>,
    /// Current text in the input box.
    pub input: String,
    /// Cursor byte-offset within `input`.
    pub input_cursor: usize,
    /// Vertical scroll offset in the chat area.
    pub scroll_offset: u16,
    /// Whether the assistant is currently streaming a response.
    pub is_streaming: bool,
    /// Ignore incoming stream events until the current stream ends.
    pub suppress_stream: bool,
    /// Accumulated streaming text (displayed live, moved to messages on StreamEnd).
    pub streaming_text: String,
    /// Set to true to exit the main loop.
    pub should_quit: bool,

    // -- status bar info --
    pub model: String,
    pub permission_mode: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl App {
    pub fn new(model: String, permission_mode: String) -> Self {
        App {
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            is_streaming: false,
            suppress_stream: false,
            streaming_text: String::new(),
            should_quit: false,
            model,
            permission_mode,
            input_tokens: 0,
            output_tokens: 0,
        }
    }

    /// Run the TUI event loop.
    ///
    /// * `terminal` — ratatui terminal handle.
    /// * `app_rx` — receives `AppEvent`s from the query bridge.
    /// * `user_tx` — sends user-submitted prompts back to the caller.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        mut app_rx: mpsc::Receiver<AppEvent>,
        user_tx: mpsc::Sender<String>,
    ) -> std::io::Result<()> {
        let (term_tx, mut term_rx) = mpsc::channel::<AppEvent>(64);

        tokio::task::spawn_blocking(move || {
            loop {
                match event::poll(Duration::from_millis(100)) {
                    Ok(true) => match event::read() {
                        Ok(Event::Key(key)) => {
                            if term_tx.blocking_send(AppEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Resize(w, h)) => {
                            if term_tx.blocking_send(AppEvent::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    },
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
        });

        terminal.draw(|f| ui::draw(f, self))?;

        loop {
            tokio::select! {
                terminal_event = term_rx.recv() => {
                    match terminal_event {
                        Some(AppEvent::Key(key)) => self.handle_key_event(key, &user_tx).await,
                        Some(AppEvent::Resize(w, h)) => self.handle_app_event(AppEvent::Resize(w, h)),
                        Some(other) => self.handle_app_event(other),
                        None => self.should_quit = true,
                    }
                }
                event = app_rx.recv() => {
                    match event {
                        Some(ev) => self.handle_app_event(ev),
                        None => self.should_quit = true,
                    }
                }
            }

            if self.should_quit {
                break;
            }

            terminal.draw(|f| ui::draw(f, self))?;
        }

        Ok(())
    }

    /// Process a keyboard event.
    pub async fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        user_tx: &mpsc::Sender<String>,
    ) {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                self.should_quit = true;
            }
            (_, KeyCode::Esc) => {
                if self.is_streaming {
                    self.is_streaming = false;
                    self.suppress_stream = true;
                    self.streaming_text.clear();
                    self.messages.push(ChatMessage::System(
                        "Stopped displaying current stream".into(),
                    ));
                } else {
                    self.input.clear();
                    self.input_cursor = 0;
                }
            }
            (_, KeyCode::Enter) => {
                if !self.is_streaming && !self.input.is_empty() {
                    let text = self.input.clone();
                    match user_tx.send(text.clone()).await {
                        Ok(()) => {
                            self.messages.push(ChatMessage::User(text));
                            self.input.clear();
                            self.input_cursor = 0;
                            self.scroll_offset = 0;
                            self.is_streaming = true;
                            self.suppress_stream = false;
                        }
                        Err(_) => {
                            self.messages.push(ChatMessage::System(
                                "Failed to submit prompt: receiver closed".into(),
                            ));
                        }
                    }
                }
            }
            (_, KeyCode::Backspace) => {
                if self.input_cursor > 0 {
                    let prev = self.input[..self.input_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input.replace_range(prev..self.input_cursor, "");
                    self.input_cursor = prev;
                }
            }
            (_, KeyCode::Delete) => {
                if self.input_cursor < self.input.len() {
                    let next = self.input[self.input_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.input_cursor + i)
                        .unwrap_or(self.input.len());
                    self.input.replace_range(self.input_cursor..next, "");
                }
            }
            (_, KeyCode::Left) => {
                if self.input_cursor > 0 {
                    self.input_cursor = self.input[..self.input_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            (_, KeyCode::Right) => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor = self.input[self.input_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.input_cursor + i)
                        .unwrap_or(self.input.len());
                }
            }
            (_, KeyCode::Home) => {
                self.input_cursor = 0;
            }
            (_, KeyCode::End) => {
                self.input_cursor = self.input.len();
            }
            (_, KeyCode::Up) => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            (_, KeyCode::Down) => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.input.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
            }
            _ => {}
        }
    }

    /// Process an event from the query bridge / background tasks.
    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::StreamDelta(text) => {
                if self.is_streaming && !self.suppress_stream {
                    self.streaming_text.push_str(&text);
                }
            }
            AppEvent::StreamEnd => {
                if self.is_streaming && !self.suppress_stream && !self.streaming_text.is_empty() {
                    let text = std::mem::take(&mut self.streaming_text);
                    self.messages.push(ChatMessage::Assistant(text));
                } else {
                    self.streaming_text.clear();
                }
                self.is_streaming = false;
                self.suppress_stream = false;
                self.scroll_offset = 0;
            }
            AppEvent::ToolUseStart { name, input } => {
                let summary = summarize_tool_input(&name, &input);
                self.messages.push(ChatMessage::ToolUse {
                    name,
                    input_summary: summary,
                });
            }
            AppEvent::ToolResult {
                name,
                output,
                is_error,
            } => {
                let summary = truncate(&output, 200);
                self.messages.push(ChatMessage::ToolResult {
                    name,
                    output_summary: summary,
                    is_error,
                });
            }
            AppEvent::AssistantMessage(text) => {
                if !self.suppress_stream {
                    self.messages.push(ChatMessage::Assistant(text));
                }
                self.streaming_text.clear();
                self.is_streaming = false;
                self.suppress_stream = false;
                self.scroll_offset = 0;
            }
            AppEvent::UsageUpdate {
                input_tokens,
                output_tokens,
            } => {
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
            }
            AppEvent::Error(msg) => {
                self.messages.push(ChatMessage::System(msg));
                self.streaming_text.clear();
                self.is_streaming = false;
                self.suppress_stream = false;
            }
            AppEvent::Resize(_, _) => {}
            AppEvent::Key(_) => {}
        }
    }
}

/// Build a short summary of tool input for display.
fn summarize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 120))
            .unwrap_or_default(),
        "FileRead" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string(),
        "FileEdit" => {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("(unknown)");
            format!("{path} (edit)")
        }
        "FileWrite" => {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("(unknown)");
            format!("{path} (write)")
        }
        _ => truncate(&input.to_string(), 80),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.min(s.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[tokio::test]
    async fn test_ctrl_c_sets_should_quit() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key_ctrl(KeyCode::Char('c')), &tx).await;
        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn test_typing_appends_to_input() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Char('h')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('i')), &tx).await;
        assert_eq!(app.input, "hi");
        assert_eq!(app.input_cursor, 2);
    }

    #[tokio::test]
    async fn test_backspace_deletes_char() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.input = "abc".into();
        app.input_cursor = 3;
        app.handle_key_event(key(KeyCode::Backspace), &tx).await;
        assert_eq!(app.input, "ab");
        assert_eq!(app.input_cursor, 2);
    }

    #[tokio::test]
    async fn test_enter_submits_input() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, mut rx) = mpsc::channel(1);
        app.input = "hello".into();
        app.input_cursor = 5;
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(app.input.is_empty());
        assert!(app.is_streaming);
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(&app.messages[0], ChatMessage::User(s) if s == "hello"));

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, "hello");
    }

    #[tokio::test]
    async fn test_enter_send_failure_preserves_input() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        app.input = "hello".into();
        app.input_cursor = 5;

        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert_eq!(app.input, "hello");
        assert!(!app.is_streaming);
        assert!(matches!(app.messages.last(), Some(ChatMessage::System(msg)) if msg.contains("Failed to submit prompt")));
    }

    #[tokio::test]
    async fn test_enter_does_nothing_when_streaming() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.input = "hello".into();
        app.is_streaming = true;
        app.handle_key_event(key(KeyCode::Enter), &tx).await;
        assert_eq!(app.input, "hello");
    }

    #[tokio::test]
    async fn test_esc_clears_input() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.input = "hello".into();
        app.input_cursor = 5;
        app.handle_key_event(key(KeyCode::Esc), &tx).await;
        assert!(app.input.is_empty());
        assert_eq!(app.input_cursor, 0);
    }

    #[tokio::test]
    async fn test_esc_suppresses_incoming_stream() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.is_streaming = true;
        app.streaming_text = "partial".into();

        app.handle_key_event(key(KeyCode::Esc), &tx).await;
        app.handle_app_event(AppEvent::StreamDelta(" ignored".into()));
        app.handle_app_event(AppEvent::AssistantMessage("ignored final".into()));

        assert!(!app.is_streaming);
        assert!(app.streaming_text.is_empty());
        assert!(matches!(app.messages.first(), Some(ChatMessage::System(msg)) if msg.contains("Stopped displaying")));
        assert!(!app.messages.iter().any(|m| matches!(m, ChatMessage::Assistant(text) if text == "ignored final")));
    }

    #[tokio::test]
    async fn test_arrow_keys_move_cursor() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.input = "abc".into();
        app.input_cursor = 3;

        app.handle_key_event(key(KeyCode::Left), &tx).await;
        assert_eq!(app.input_cursor, 2);
        app.handle_key_event(key(KeyCode::Left), &tx).await;
        assert_eq!(app.input_cursor, 1);
        app.handle_key_event(key(KeyCode::Right), &tx).await;
        assert_eq!(app.input_cursor, 2);
        app.handle_key_event(key(KeyCode::Home), &tx).await;
        assert_eq!(app.input_cursor, 0);
        app.handle_key_event(key(KeyCode::End), &tx).await;
        assert_eq!(app.input_cursor, 3);
    }

    #[tokio::test]
    async fn test_up_down_scrolls() {
        let mut app = App::new("test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Up), &tx).await;
        assert_eq!(app.scroll_offset, 1);
        app.handle_key_event(key(KeyCode::Up), &tx).await;
        assert_eq!(app.scroll_offset, 2);
        app.handle_key_event(key(KeyCode::Down), &tx).await;
        assert_eq!(app.scroll_offset, 1);
    }

    #[test]
    fn test_stream_delta_and_end() {
        let mut app = App::new("test".into(), "default".into());
        app.is_streaming = true;
        app.handle_app_event(AppEvent::StreamDelta("hello ".into()));
        app.handle_app_event(AppEvent::StreamDelta("world".into()));
        assert_eq!(app.streaming_text, "hello world");
        assert!(app.is_streaming);

        app.handle_app_event(AppEvent::StreamEnd);
        assert!(!app.is_streaming);
        assert!(app.streaming_text.is_empty());
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(&app.messages[0], ChatMessage::Assistant(s) if s == "hello world"));
    }

    #[test]
    fn test_tool_events() {
        let mut app = App::new("test".into(), "default".into());
        app.handle_app_event(AppEvent::ToolUseStart {
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls -la"}),
        });
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(&app.messages[0], ChatMessage::ToolUse { name, input_summary } if name == "Bash" && input_summary == "ls -la"));

        app.handle_app_event(AppEvent::ToolResult {
            name: "Bash".into(),
            output: "file1\nfile2".into(),
            is_error: false,
        });
        assert_eq!(app.messages.len(), 2);
    }

    #[test]
    fn test_usage_update() {
        let mut app = App::new("test".into(), "default".into());
        app.handle_app_event(AppEvent::UsageUpdate {
            input_tokens: 100,
            output_tokens: 50,
        });
        assert_eq!(app.input_tokens, 100);
        assert_eq!(app.output_tokens, 50);
    }

    #[test]
    fn test_error_event() {
        let mut app = App::new("test".into(), "default".into());
        app.is_streaming = true;
        app.handle_app_event(AppEvent::Error("something failed".into()));
        assert!(!app.is_streaming);
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(&app.messages[0], ChatMessage::System(s) if s == "something failed"));
    }

    #[test]
    fn test_summarize_tool_input_bash() {
        let input = serde_json::json!({"command": "echo hello"});
        assert_eq!(summarize_tool_input("Bash", &input), "echo hello");
    }

    #[test]
    fn test_summarize_tool_input_file_read() {
        let input = serde_json::json!({"file_path": "/tmp/test.rs"});
        assert_eq!(summarize_tool_input("FileRead", &input), "/tmp/test.rs");
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let long = "a".repeat(100);
        let result = truncate(&long, 10);
        assert_eq!(result.len(), 13);
        assert!(result.ends_with("..."));
    }
}
