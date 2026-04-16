use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use rust_claude_core::state::TodoItem;
use tokio::sync::mpsc;

use crate::events::{AppEvent, ChatMessage, PermissionResponse};
use crate::ui;

/// State of the modal permission confirmation dialog.
pub struct PermissionDialog {
    /// The tool requesting permission.
    pub tool_name: String,
    /// Summary of the tool input.
    pub input_summary: String,
    /// Which option is currently highlighted (0=Allow, 1=AlwaysAllow, 2=Deny, 3=AlwaysDeny).
    pub selected: usize,
    /// Channel to send the user's decision back to the query loop.
    pub response_tx: Option<tokio::sync::oneshot::Sender<PermissionResponse>>,
}

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
    /// Whether the model is in the "thinking" phase.
    pub is_thinking: bool,
    /// Ignore incoming stream events until the current stream ends.
    pub suppress_stream: bool,
    /// Accumulated streaming text (displayed live, moved to messages on StreamEnd).
    pub streaming_text: String,
    /// Set to true to exit the main loop.
    pub should_quit: bool,

    // -- status bar info --
    pub model: String,
    pub model_setting: String,
    pub permission_mode: String,
    pub input_tokens: u64,
    pub output_tokens: u64,

    // -- permission dialog --
    pub permission_dialog: Option<PermissionDialog>,

    // -- todo panel --
    pub todo_visible: bool,
    pub todos: Vec<TodoItem>,
}

impl App {
    pub fn new(model: String, model_setting: String, permission_mode: String) -> Self {
        App {
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            is_streaming: false,
            is_thinking: false,
            suppress_stream: false,
            streaming_text: String::new(),
            should_quit: false,
            model,
            model_setting,
            permission_mode,
            input_tokens: 0,
            output_tokens: 0,
            permission_dialog: None,
            todo_visible: false,
            todos: Vec::new(),
        }
    }

    /// Run the TUI event loop.
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
        if self.permission_dialog.is_some() {
            self.handle_permission_key(key);
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match key.code {
            KeyCode::Enter => {
                if self.is_streaming {
                    return;
                }

                if self.input.trim_start().starts_with('/') {
                    self.handle_slash_command(user_tx).await;
                    return;
                }

                let prompt = self.input.trim().to_string();
                if prompt.is_empty() {
                    return;
                }

                match user_tx.send(prompt.clone()).await {
                    Ok(()) => {
                        self.messages.push(ChatMessage::User(prompt));
                        self.input.clear();
                        self.input_cursor = 0;
                        self.is_streaming = true;
                        self.is_thinking = false;
                        self.suppress_stream = false;
                    }
                    Err(_) => {
                        self.messages.push(ChatMessage::System(
                            "Failed to submit prompt to background worker.".into(),
                        ));
                    }
                }
            }
            KeyCode::Char(c) if !self.is_streaming => {
                self.input.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
            }
            KeyCode::Backspace if !self.is_streaming => {
                if self.input_cursor > 0 {
                    let prev = self.input[..self.input_cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input.replace_range(prev..self.input_cursor, "");
                    self.input_cursor = prev;
                }
            }
            KeyCode::Delete if !self.is_streaming => {
                if self.input_cursor < self.input.len() {
                    let next = self.input[self.input_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.input_cursor + i)
                        .unwrap_or(self.input.len());
                    self.input.replace_range(self.input_cursor..next, "");
                }
            }
            KeyCode::Left if !self.is_streaming => {
                if self.input_cursor > 0 {
                    self.input_cursor = self.input[..self.input_cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right if !self.is_streaming => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor = self.input[self.input_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.input_cursor + i)
                        .unwrap_or(self.input.len());
                }
            }
            KeyCode::Home if !self.is_streaming => {
                self.input_cursor = 0;
            }
            KeyCode::End if !self.is_streaming => {
                self.input_cursor = self.input.len();
            }
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Esc => {
                if self.is_streaming {
                    self.suppress_stream = true;
                    self.is_streaming = false;
                    self.streaming_text.clear();
                    self.messages.push(ChatMessage::System(
                        "Stopped displaying current response.".into(),
                    ));
                } else {
                    self.input.clear();
                    self.input_cursor = 0;
                }
            }
            _ => {}
        }
    }

    fn handle_permission_key(&mut self, key: crossterm::event::KeyEvent) {
        let dialog = match self.permission_dialog.as_mut() {
            Some(d) => d,
            None => return,
        };

        let options_len = 4usize;
        match key.code {
            KeyCode::Up => dialog.selected = dialog.selected.saturating_sub(1),
            KeyCode::Down => dialog.selected = (dialog.selected + 1).min(options_len - 1),
            KeyCode::Esc | KeyCode::Char('n') => self.finish_permission_dialog(PermissionResponse::Deny),
            KeyCode::Char('y') => self.finish_permission_dialog(PermissionResponse::Allow),
            KeyCode::Char('a') => self.finish_permission_dialog(PermissionResponse::AlwaysAllow),
            KeyCode::Char('d') => self.finish_permission_dialog(PermissionResponse::AlwaysDeny),
            KeyCode::Enter => {
                let response = match dialog.selected {
                    0 => PermissionResponse::Allow,
                    1 => PermissionResponse::AlwaysAllow,
                    2 => PermissionResponse::Deny,
                    _ => PermissionResponse::AlwaysDeny,
                };
                self.finish_permission_dialog(response);
            }
            _ => {}
        }
    }

    fn finish_permission_dialog(&mut self, response: PermissionResponse) {
        if let Some(dialog) = self.permission_dialog.take() {
            if let Some(tx) = dialog.response_tx {
                let _ = tx.send(response);
            }
        }
    }

    async fn handle_slash_command(&mut self, user_tx: &mpsc::Sender<String>) {
        let cmd = self.input.trim().to_string();
        self.input.clear();
        self.input_cursor = 0;

        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        match parts[0] {
            "/clear" => {
                self.messages.clear();
                self.streaming_text.clear();
                self.scroll_offset = 0;
                self.messages.push(ChatMessage::System("Session cleared.".into()));
            }
            "/compact" => match user_tx.send("[COMPACT_REQUEST]".to_string()).await {
                Ok(()) => self.messages.push(ChatMessage::System(
                    "Compacting conversation history...".into(),
                )),
                Err(_) => self.messages.push(ChatMessage::System(
                    "Error: failed to send compact request".into(),
                )),
            },
            "/mode" => {
                if parts.len() > 1 {
                    let mode_str = parts[1].trim();
                    match mode_str {
                        "default" | "accept-edits" | "bypass" | "plan" | "dont-ask" => {
                            let control = format!("[MODE_REQUEST]:{mode_str}");
                            match user_tx.send(control).await {
                                Ok(()) => self.messages.push(ChatMessage::System(format!(
                                    "Switching permission mode to: {mode_str}"
                                ))),
                                Err(_) => self.messages.push(ChatMessage::System(
                                    "Error: failed to send mode change request".into(),
                                )),
                            }
                        }
                        _ => self.messages.push(ChatMessage::System(
                            "Unknown mode. Valid modes: default, accept-edits, bypass, plan, dont-ask".into(),
                        )),
                    }
                } else {
                    self.messages.push(ChatMessage::System(format!(
                        "Current mode: {}. Usage: /mode <default|accept-edits|bypass|plan|dont-ask>",
                        self.permission_mode
                    )));
                }
            }
            "/model" => {
                if parts.len() > 1 {
                    let model_str = parts[1].trim();
                    let control = format!("[MODEL_REQUEST]:{model_str}");
                    match user_tx.send(control).await {
                        Ok(()) => self.messages.push(ChatMessage::System(format!(
                            "Switching model to: {model_str}"
                        ))),
                        Err(_) => self.messages.push(ChatMessage::System(
                            "Error: failed to send model change request".into(),
                        )),
                    }
                } else {
                    self.messages.push(ChatMessage::System(format!(
                        "Current model setting: {}\nCurrent runtime model: {}",
                        self.model_setting, self.model
                    )));
                }
            }
            "/todo" => self.todo_visible = !self.todo_visible,
            "/help" => self.messages.push(ChatMessage::System(
                "Available commands:\n  /clear       — Clear current session\n  /compact     — Compact conversation history\n  /mode <mode> — Switch permission mode\n  /model <m>   — Switch model setting\n  /todo        — Toggle todo panel\n  /help        — Show this help\n  /exit        — Exit".into(),
            )),
            "/exit" => self.should_quit = true,
            _ => self.messages.push(ChatMessage::System(format!(
                "Unknown command: {}. Type /help for available commands.",
                parts[0]
            ))),
        }
    }

    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::ThinkingStart => self.is_thinking = true,
            AppEvent::StreamDelta(text) => {
                if self.is_streaming && !self.suppress_stream {
                    self.is_thinking = false;
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
                self.is_thinking = false;
                self.suppress_stream = false;
                self.scroll_offset = 0;
            }
            AppEvent::ToolUseStart { name, input } => {
                self.is_thinking = false;
                let summary = summarize_tool_input(&name, &input);
                self.messages.push(ChatMessage::ToolUse {
                    name,
                    input_summary: summary,
                });
            }
            AppEvent::ToolResult { name, output, is_error } => {
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
                self.is_thinking = false;
                self.suppress_stream = false;
                self.scroll_offset = 0;
            }
            AppEvent::UsageUpdate { input_tokens, output_tokens } => {
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
            }
            AppEvent::StatusUpdate {
                model,
                model_setting,
                permission_mode,
            } => {
                self.model = model;
                self.model_setting = model_setting;
                self.permission_mode = permission_mode;
            }
            AppEvent::Error(msg) => {
                self.messages.push(ChatMessage::System(msg));
                self.streaming_text.clear();
                self.is_streaming = false;
                self.is_thinking = false;
                self.suppress_stream = false;
            }
            AppEvent::PermissionRequest { tool_name, input, response_tx } => {
                let input_summary = summarize_tool_input(&tool_name, &input);
                self.permission_dialog = Some(PermissionDialog {
                    tool_name,
                    input_summary,
                    selected: 0,
                    response_tx: Some(response_tx),
                });
            }
            AppEvent::TodoUpdate(todos) => self.todos = todos,
            AppEvent::CompactionStart => {
                self.messages.push(ChatMessage::System(
                    "Compacting conversation history...".into(),
                ));
            }
            AppEvent::CompactionComplete { result } => {
                self.messages.push(ChatMessage::System(format!(
                    "Compacted {} messages into summary. Preserved {} recent messages. Estimated tokens: {}K -> {}K",
                    result.compacted_message_count,
                    result.preserved_message_count,
                    result.estimated_tokens_before / 1000,
                    result.estimated_tokens_after / 1000,
                )));
            }
            AppEvent::Resize(_, _) | AppEvent::Key(_) => {}
        }
    }
}

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
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key_ctrl(KeyCode::Char('c')), &tx).await;
        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn test_typing_appends_to_input() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into());
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Char('h')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('i')), &tx).await;
        assert_eq!(app.input, "hi");
        assert_eq!(app.input_cursor, 2);
    }

    #[tokio::test]
    async fn test_mode_command_sends_control_message() {
        let mut app = App::new("claude-sonnet-4-6".into(), "opusplan".into(), "Default".into());
        let (tx, mut rx) = mpsc::channel(1);
        app.input = "/mode plan".into();
        app.input_cursor = app.input.len();
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, "[MODE_REQUEST]:plan");
    }

    #[tokio::test]
    async fn test_model_command_sends_control_message() {
        let mut app = App::new("claude-sonnet-4-6".into(), "opusplan".into(), "Default".into());
        let (tx, mut rx) = mpsc::channel(1);
        app.input = "/model opus[1m]".into();
        app.input_cursor = app.input.len();
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, "[MODEL_REQUEST]:opus[1m]");
    }

    #[tokio::test]
    async fn test_model_command_without_args_shows_setting_and_runtime() {
        let mut app = App::new("claude-opus-4-6".into(), "opusplan".into(), "Plan".into());
        let (tx, mut rx) = mpsc::channel(1);
        app.input = "/model".into();
        app.input_cursor = app.input.len();
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg))
                if msg.contains("Current model setting: opusplan")
                    && msg.contains("Current runtime model: claude-opus-4-6")
        ));
    }

    #[test]
    fn test_status_update_changes_displayed_model_and_mode() {
        let mut app = App::new("claude-sonnet-4-6".into(), "opusplan".into(), "Default".into());
        app.handle_app_event(AppEvent::StatusUpdate {
            model: "claude-opus-4-6".into(),
            model_setting: "opusplan".into(),
            permission_mode: "Plan".into(),
        });

        assert_eq!(app.model, "claude-opus-4-6");
        assert_eq!(app.model_setting, "opusplan");
        assert_eq!(app.permission_mode, "Plan");
    }
}
