use std::fs;
use std::io::Stdout;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use rust_claude_core::config::Theme;
use rust_claude_core::state::TodoItem;
use tokio::sync::mpsc;

use crate::events::{AppEvent, ChatMessage, PermissionResponse, UserCommand};
use crate::ui;

const CHAT_SCROLL_PAGE_SIZE: u16 = 8;

const MAX_HISTORY_ENTRIES: usize = 500;

struct SlashCommandSpec {
    name: &'static str,
    usage: &'static str,
    description: &'static str,
}

const SLASH_COMMANDS: &[SlashCommandSpec] = &[
    SlashCommandSpec {
        name: "/clear",
        usage: "/clear [keep-context]",
        description: "Clear transcript (optionally preserve context)",
    },
    SlashCommandSpec {
        name: "/compact",
        usage: "/compact",
        description: "Compact conversation history",
    },
    SlashCommandSpec {
        name: "/config",
        usage: "/config",
        description: "Show effective config sources",
    },
    SlashCommandSpec {
        name: "/cost",
        usage: "/cost",
        description: "Show session token usage and cost estimate",
    },
    SlashCommandSpec {
        name: "/diff",
        usage: "/diff",
        description: "Show current git working tree diff",
    },
    SlashCommandSpec {
        name: "/mode",
        usage: "/mode <mode>",
        description: "Switch permission mode",
    },
    SlashCommandSpec {
        name: "/memory",
        usage: "/memory [remember|forget] ...",
        description: "Inspect or maintain current memory store",
    },
    SlashCommandSpec {
        name: "/model",
        usage: "/model <model>",
        description: "Switch model setting",
    },
    SlashCommandSpec {
        name: "/theme",
        usage: "/theme <dark|light>",
        description: "Switch TUI theme",
    },
    SlashCommandSpec {
        name: "/todo",
        usage: "/todo",
        description: "Toggle todo panel",
    },
    SlashCommandSpec {
        name: "/hooks",
        usage: "/hooks",
        description: "Show configured hooks",
    },
    SlashCommandSpec {
        name: "/mcp",
        usage: "/mcp",
        description: "Show MCP server status and tools",
    },
    SlashCommandSpec {
        name: "/help",
        usage: "/help",
        description: "Show this help",
    },
    SlashCommandSpec {
        name: "/exit",
        usage: "/exit",
        description: "Exit",
    },
];

fn slash_command_help_text() -> String {
    let mut text = String::from("Available commands:\n");
    for command in SLASH_COMMANDS {
        text.push_str(&format!("  {:<22} — {}\n", command.usage, command.description));
    }
    text.push_str(
        "\nEditing:\n  Enter submits\n  Shift+Enter inserts newline\n  Up/Down browse history or move multi-line cursor\n  PageUp/PageDown scroll chat history\n  Ctrl+Home/Ctrl+End jump to oldest/latest chat content\n  Ctrl+A/Ctrl+E/Home/End move within line\n  Ctrl+Left/Ctrl+Right move by word\n  Escape or Ctrl+C cancels active stream\n  Ctrl+L redraws the screen\n  Tab toggles latest thinking block",
    );
    text
}

fn has_slash_command(name: &str) -> bool {
    SLASH_COMMANDS.iter().any(|command| command.name == name)
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBuffer {
    lines: Vec<String>,
    cursor: CursorPosition,
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: CursorPosition { row: 0, col: 0 },
        }
    }
}

impl InputBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_text(text: &str) -> Self {
        let mut lines: Vec<String> = text.split('\n').map(|line| line.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let row = lines.len().saturating_sub(1);
        let col = lines[row].chars().count();
        Self {
            lines,
            cursor: CursorPosition { row, col },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn current_line(&self) -> &str {
        &self.lines[self.cursor.row]
    }

    pub fn cursor(&self) -> CursorPosition {
        self.cursor
    }

    pub fn set_cursor(&mut self, row: usize, col: usize) {
        let row = row.min(self.lines.len().saturating_sub(1));
        let col = col.min(self.lines[row].chars().count());
        self.cursor = CursorPosition { row, col };
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.lines.push(String::new());
        self.cursor = CursorPosition { row: 0, col: 0 };
    }

    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    fn char_to_byte(s: &str, char_idx: usize) -> usize {
        if char_idx == 0 {
            return 0;
        }
        s.char_indices()
            .nth(char_idx)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len())
    }

    fn current_line_len(&self) -> usize {
        self.lines[self.cursor.row].chars().count()
    }

    pub fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.cursor.row];
        let byte_idx = Self::char_to_byte(line, self.cursor.col);
        line.insert(byte_idx, c);
        self.cursor.col += 1;
    }

    pub fn insert_text(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(ch);
            }
        }
    }

    pub fn insert_newline(&mut self) {
        let current = &mut self.lines[self.cursor.row];
        let split_at = Self::char_to_byte(current, self.cursor.col);
        let tail = current.split_off(split_at);
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.lines.insert(self.cursor.row, tail);
    }

    pub fn backspace(&mut self) {
        if self.cursor.col > 0 {
            let line = &mut self.lines[self.cursor.row];
            let end = Self::char_to_byte(line, self.cursor.col);
            let start = Self::char_to_byte(line, self.cursor.col - 1);
            line.replace_range(start..end, "");
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            let current = self.lines.remove(self.cursor.row);
            self.cursor.row -= 1;
            let prev_len = self.lines[self.cursor.row].chars().count();
            self.lines[self.cursor.row].push_str(&current);
            self.cursor.col = prev_len;
        }
    }

    pub fn delete(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor.col < line_len {
            let line = &mut self.lines[self.cursor.row];
            let start = Self::char_to_byte(line, self.cursor.col);
            let end = Self::char_to_byte(line, self.cursor.col + 1);
            line.replace_range(start..end, "");
        } else if self.cursor.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor.row + 1);
            self.lines[self.cursor.row].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.lines[self.cursor.row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        } else if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.cursor.col.min(self.current_line_len());
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = self.cursor.col.min(self.current_line_len());
        }
    }

    pub fn move_home(&mut self) {
        self.cursor.col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor.col = self.current_line_len();
    }

    pub fn move_word_left(&mut self) {
        if self.cursor.col == 0 {
            if self.cursor.row > 0 {
                self.cursor.row -= 1;
                self.cursor.col = self.current_line_len();
            }
            return;
        }

        let chars: Vec<char> = self.current_line().chars().collect();
        let mut pos = self.cursor.col;
        while pos > 0 && chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        self.cursor.col = pos;
    }

    pub fn move_word_right(&mut self) {
        let chars: Vec<char> = self.current_line().chars().collect();
        let mut pos = self.cursor.col;
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        while pos < chars.len() && !chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos == chars.len() && self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        } else {
            self.cursor.col = pos;
        }
    }
}

/// State for a tool call being constructed during streaming.
#[derive(Debug, Clone)]
pub struct StreamingToolState {
    /// The tool name.
    pub name: String,
    /// Accumulated partial JSON input.
    pub accumulated_json: String,
}

/// Main TUI application state.
pub struct App {
    /// Chat message history.
    pub messages: Vec<ChatMessage>,
    /// Input buffer with multi-line editing support.
    pub input_buffer: InputBuffer,
    /// Vertical scroll offset in the chat area.
    pub scroll_offset: u16,
    /// Whether chat viewport should stay pinned to latest output.
    pub follow_output: bool,
    /// Whether the assistant is currently streaming a response.
    pub is_streaming: bool,
    /// Whether the model is in the "thinking" phase.
    pub is_thinking: bool,
    /// Ignore incoming stream events until the current stream ends.
    pub suppress_stream: bool,
    /// Accumulated streaming text (displayed live, moved to messages on StreamEnd).
    pub streaming_text: String,
    /// Raw text accumulator for conversion to ChatMessage on stream end.
    pub streaming_text_raw: String,
    /// Incremental markdown state for streaming text rendering.
    pub streaming_md: crate::streaming_markdown::StreamingMarkdownState,
    /// Accumulated streaming thinking text.
    pub streaming_thinking: String,
    /// Incremental markdown state for streaming thinking rendering.
    pub streaming_thinking_md: crate::streaming_markdown::StreamingMarkdownState,
    /// Whether the thinking block is currently folded during streaming.
    pub thinking_folded: bool,
    /// State for a tool call being constructed during streaming.
    pub streaming_tool: Option<StreamingToolState>,
    /// Expanded thinking message indices.
    pub expanded_thinking: Vec<usize>,
    /// Selected thinking block for keyboard expand/collapse.
    pub selected_thinking: Option<usize>,
    /// Whether current visible area should be cleared/redrawn.
    pub clear_requested: bool,
    /// Set to true to exit the main loop.
    pub should_quit: bool,

    // -- status bar info --
    pub model: String,
    pub model_setting: String,
    pub permission_mode: String,
    pub git_branch: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,

    // -- permission dialog --
    pub permission_dialog: Option<PermissionDialog>,

    // -- task panel --
    pub todo_visible: bool,
    pub tasks: Vec<TodoItem>,

    // -- history --
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub draft_before_history: Option<InputBuffer>,
    pub history_path: PathBuf,

    // -- terminal dimensions (updated on every draw / resize) --
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub theme: Theme,
}

impl App {
    pub fn palette(&self) -> crate::theme::Palette {
        crate::theme::Palette::from_config(self.theme)
    }

    pub fn new(
        model: String,
        model_setting: String,
        permission_mode: String,
        git_branch: Option<String>,
        theme: Theme,
    ) -> Self {
        let history_path = history_file_path();
        let history = load_history(&history_path);
        App {
            messages: Vec::new(),
            input_buffer: InputBuffer::new(),
            scroll_offset: 0,
            follow_output: true,
            is_streaming: false,
            is_thinking: false,
            suppress_stream: false,
            streaming_text: String::new(),
            streaming_text_raw: String::new(),
            streaming_md: crate::streaming_markdown::StreamingMarkdownState::new(
                crate::theme::Palette::from_config(theme),
            ),
            streaming_thinking: String::new(),
            streaming_thinking_md: crate::streaming_markdown::StreamingMarkdownState::new(
                crate::theme::Palette::from_config(theme),
            ),
            thinking_folded: false,
            streaming_tool: None,
            expanded_thinking: Vec::new(),
            selected_thinking: None,
            clear_requested: false,
            should_quit: false,
            model,
            model_setting,
            permission_mode,
            git_branch,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
            permission_dialog: None,
            todo_visible: false,
            tasks: Vec::new(),
            history,
            history_index: None,
            draft_before_history: None,
            history_path,
            terminal_width: 80,
            terminal_height: 24,
            theme,
        }
    }

    pub fn input_text(&self) -> String {
        self.input_buffer.to_text()
    }

    pub fn input_cursor(&self) -> CursorPosition {
        self.input_buffer.cursor()
    }

    fn reset_input_navigation(&mut self) {
        self.history_index = None;
        self.draft_before_history = None;
    }

    fn max_chat_scroll_offset(&self) -> u16 {
        let viewport = ui::chat_viewport_area(
            self,
            Rect::new(0, 0, self.terminal_width, self.terminal_height),
        );
        ui::max_chat_scroll_offset(self, viewport.width, viewport.height)
    }

    fn clamp_chat_scroll(&mut self) {
        let max_offset = self.max_chat_scroll_offset();
        self.scroll_offset = self.scroll_offset.min(max_offset);
        if self.scroll_offset >= max_offset {
            self.scroll_offset = max_offset;
            self.follow_output = true;
        }
    }

    fn jump_chat_to_latest(&mut self) {
        self.scroll_offset = self.max_chat_scroll_offset();
        self.follow_output = true;
    }

    fn jump_chat_to_oldest(&mut self) {
        self.scroll_offset = 0;
        self.follow_output = self.max_chat_scroll_offset() == 0;
    }

    fn scroll_chat_up(&mut self) {
        let max_offset = self.max_chat_scroll_offset();
        if max_offset == 0 {
            self.scroll_offset = 0;
            self.follow_output = true;
            return;
        }

        if self.follow_output {
            self.scroll_offset = max_offset;
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(CHAT_SCROLL_PAGE_SIZE);
        self.follow_output = false;
    }

    fn scroll_chat_down(&mut self) {
        let max_offset = self.max_chat_scroll_offset();
        self.scroll_offset = (self.scroll_offset + CHAT_SCROLL_PAGE_SIZE).min(max_offset);
        self.follow_output = self.scroll_offset >= max_offset;
    }

    fn scroll_chat_up_lines(&mut self, lines: u16) {
        let max_offset = self.max_chat_scroll_offset();
        if max_offset == 0 {
            self.scroll_offset = 0;
            self.follow_output = true;
            return;
        }

        if self.follow_output {
            self.scroll_offset = max_offset;
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.follow_output = false;
    }

    fn scroll_chat_down_lines(&mut self, lines: u16) {
        let max_offset = self.max_chat_scroll_offset();
        if self.follow_output {
            self.scroll_offset = max_offset;
        }
        self.scroll_offset = self.scroll_offset.saturating_add(lines).min(max_offset);
        self.follow_output = self.scroll_offset >= max_offset;
    }

    fn sync_chat_viewport(&mut self) {
        let max_offset = self.max_chat_scroll_offset();
        if self.follow_output {
            self.scroll_offset = max_offset;
        } else {
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }
        self.follow_output = self.scroll_offset >= max_offset;
    }

    fn push_history_entry(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.history.last().is_some_and(|last| last == trimmed) {
            self.reset_input_navigation();
            return;
        }
        self.history.push(trimmed.to_string());
        if self.history.len() > MAX_HISTORY_ENTRIES {
            let overflow = self.history.len() - MAX_HISTORY_ENTRIES;
            self.history.drain(0..overflow);
        }
        let _ = save_history(&self.history_path, &self.history);
        self.reset_input_navigation();
    }

    fn browse_history_older(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.draft_before_history = Some(self.input_buffer.clone());
            self.history_index = Some(self.history.len().saturating_sub(1));
        } else if let Some(index) = self.history_index {
            self.history_index = Some(index.saturating_sub(1));
        }
        if let Some(index) = self.history_index {
            self.input_buffer = InputBuffer::from_text(&self.history[index]);
        }
    }

    fn browse_history_newer(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            let next = index + 1;
            self.history_index = Some(next);
            self.input_buffer = InputBuffer::from_text(&self.history[next]);
        } else {
            self.history_index = None;
            self.input_buffer = self.draft_before_history.clone().unwrap_or_default();
            self.draft_before_history = None;
        }
    }

    fn toggle_selected_thinking(&mut self) {
        let Some(index) = self.selected_thinking else {
            return;
        };
        if let Some(pos) = self.expanded_thinking.iter().position(|&i| i == index) {
            self.expanded_thinking.remove(pos);
        } else {
            self.expanded_thinking.push(index);
        }
    }

    fn update_selected_thinking(&mut self) {
        self.selected_thinking = self
            .messages
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, msg)| matches!(msg, ChatMessage::Thinking { .. }).then_some(idx));
    }

    async fn submit_input(&mut self, user_tx: &mpsc::Sender<UserCommand>) {
        let full_input = self.input_text();
        if full_input.trim().is_empty() {
            return;
        }

        if full_input.trim_start().starts_with('/') {
            self.handle_slash_command(user_tx).await;
            return;
        }

        match user_tx.send(UserCommand::Prompt(full_input.clone())).await {
            Ok(()) => {
                self.messages.push(ChatMessage::User(full_input.clone()));
                self.push_history_entry(&full_input);
                self.input_buffer.clear();
                self.is_streaming = true;
                self.is_thinking = false;
                self.suppress_stream = false;
                self.streaming_text.clear();
                self.streaming_thinking.clear();
                self.follow_output = true;
                self.sync_chat_viewport();
            }
            Err(_) => {
                self.messages.push(ChatMessage::System(
                    "Failed to submit prompt to background worker.".into(),
                ));
                self.sync_chat_viewport();
            }
        }
    }

    fn cancel_stream_local(&mut self) {
        // Finalize thinking content as cancelled if any
        if !self.streaming_thinking.is_empty() {
            let thinking_text = std::mem::take(&mut self.streaming_thinking);
            self.streaming_thinking_md.clear();
            let summary = format!("Thought for ~{} chars (cancelled)", thinking_text.chars().count());
            self.messages.push(ChatMessage::Thinking {
                summary,
                content: thinking_text,
            });
            self.update_selected_thinking();
        }
        self.suppress_stream = true;
        self.is_streaming = false;
        self.is_thinking = false;
        self.streaming_text.clear();
        self.streaming_text_raw.clear();
        self.streaming_md.clear();
        self.streaming_thinking.clear();
        self.streaming_thinking_md.clear();
        self.streaming_tool = None;
        self.messages.push(ChatMessage::System(
            "Cancelled current response.".into(),
        ));
        self.sync_chat_viewport();
    }

    /// Run the TUI event loop.
    ///
    /// During streaming, rendering is rate-limited to ~30 FPS (33ms tick)
    /// to batch rapid delta events and prevent flicker. When not streaming,
    /// events trigger immediate redraws.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        mut app_rx: mpsc::Receiver<AppEvent>,
        user_tx: mpsc::Sender<UserCommand>,
    ) -> std::io::Result<()> {
        let (term_tx, mut term_rx) = mpsc::channel::<AppEvent>(64);

        tokio::task::spawn_blocking(move || loop {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key)) => {
                        if term_tx.blocking_send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Mouse(mouse)) => {
                        if term_tx.blocking_send(AppEvent::Mouse(mouse)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Paste(text)) => {
                        if term_tx.blocking_send(AppEvent::Paste(text)).is_err() {
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
        });

        // Capture initial terminal size.
        {
            let size = terminal.size()?;
            self.terminal_width = size.width;
            self.terminal_height = size.height;
        }
        terminal.draw(|f| ui::draw(f, self))?;

        // Frame rate limiting: 33ms tick (~30 FPS) during streaming
        let mut render_tick = tokio::time::interval(Duration::from_millis(33));
        render_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut dirty = false;

        loop {
            // Determine if we're streaming and should use tick-based rendering
            let is_streaming = self.is_streaming;

            tokio::select! {
                terminal_event = term_rx.recv() => {
                    match terminal_event {
                        Some(AppEvent::Key(key)) => self.handle_key_event(key, &user_tx).await,
                        Some(AppEvent::Mouse(mouse)) => self.handle_mouse_event(mouse),
                        Some(AppEvent::Paste(text)) => self.handle_paste(text),
                        Some(AppEvent::Resize(w, h)) => self.handle_app_event(AppEvent::Resize(w, h)),
                        Some(other) => self.handle_app_event(other),
                        None => self.should_quit = true,
                    }
                    dirty = true;
                }
                event = app_rx.recv() => {
                    match event {
                        Some(ev) => {
                            let is_delta = matches!(&ev,
                                AppEvent::StreamDelta(_) |
                                AppEvent::ThinkingDelta(_) |
                                AppEvent::ToolInputDelta { .. }
                            );
                            self.handle_app_event(ev);
                            if is_delta && is_streaming {
                                // During streaming, delta events only set dirty flag;
                                // actual redraw happens on the next tick
                                dirty = true;
                                continue;
                            }
                            dirty = true;
                        }
                        None => self.should_quit = true,
                    }
                }
                _ = render_tick.tick(), if is_streaming && dirty => {
                    // Tick fires during streaming when dirty — redraw
                    // (handled below in the common draw path)
                }
            }

            if self.should_quit {
                break;
            }

            if self.clear_requested {
                terminal.clear()?;
                self.clear_requested = false;
            }

            if dirty {
                terminal.draw(|f| ui::draw(f, self))?;
                dirty = false;
            }
        }

        Ok(())
    }

    pub fn handle_paste(&mut self, text: String) {
        if self.is_streaming {
            return;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        self.input_buffer.insert_text(&normalized);
        self.reset_input_navigation();
    }

    /// Process a keyboard event.
    pub async fn handle_key_event(
        &mut self,
        key: KeyEvent,
        user_tx: &mpsc::Sender<UserCommand>,
    ) {
        if self.permission_dialog.is_some() {
            self.handle_permission_key(key);
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            self.clear_requested = true;
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.is_streaming {
                let _ = user_tx.send(UserCommand::CancelStream).await;
                self.cancel_stream_local();
            } else {
                self.should_quit = true;
            }
            return;
        }

        match key.code {
            KeyCode::PageUp => self.scroll_chat_up(),
            KeyCode::PageDown => self.scroll_chat_down(),
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.jump_chat_to_oldest();
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.jump_chat_to_latest();
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) && !self.is_streaming => {
                self.input_buffer.insert_newline();
                self.reset_input_navigation();
            }
            KeyCode::Enter => {
                if self.is_streaming {
                    return;
                }
                self.submit_input(user_tx).await;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming => {
                self.input_buffer.move_end();
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming => {
                self.input_buffer.move_home();
            }
            KeyCode::Char(' ') if !self.is_streaming => {
                self.input_buffer.insert_char(' ');
                self.reset_input_navigation();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming => {
                self.input_buffer.insert_char(c);
                self.reset_input_navigation();
            }
            KeyCode::Backspace if !self.is_streaming => {
                self.input_buffer.backspace();
                self.reset_input_navigation();
            }
            KeyCode::Delete if !self.is_streaming => {
                self.input_buffer.delete();
                self.reset_input_navigation();
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming => {
                self.input_buffer.move_word_left();
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming => {
                self.input_buffer.move_word_right();
            }
            KeyCode::Left if !self.is_streaming => self.input_buffer.move_left(),
            KeyCode::Right if !self.is_streaming => self.input_buffer.move_right(),
            KeyCode::Home if !self.is_streaming => self.input_buffer.move_home(),
            KeyCode::End if !self.is_streaming => self.input_buffer.move_end(),
            KeyCode::Up if !self.is_streaming && self.input_buffer.line_count() > 1 => {
                self.input_buffer.move_up();
            }
            KeyCode::Down if !self.is_streaming && self.input_buffer.line_count() > 1 => {
                self.input_buffer.move_down();
            }
            KeyCode::Up if !self.is_streaming => self.browse_history_older(),
            KeyCode::Down if !self.is_streaming => self.browse_history_newer(),
            KeyCode::Esc => {
                if self.is_streaming {
                    let _ = user_tx.send(UserCommand::CancelStream).await;
                    self.cancel_stream_local();
                } else {
                    self.input_buffer.clear();
                    self.reset_input_navigation();
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if self.is_streaming && self.is_thinking {
                    // Toggle thinking fold/unfold during streaming
                    self.thinking_folded = !self.thinking_folded;
                } else if !self.is_streaming {
                    self.toggle_selected_thinking();
                }
            }
            KeyCode::Char('\t') => {
                if self.is_streaming && self.is_thinking {
                    self.thinking_folded = !self.thinking_folded;
                } else if !self.is_streaming {
                    self.toggle_selected_thinking();
                }
            }
            _ => {}
        }
        self.clamp_chat_scroll();
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        let chat_area = ui::chat_viewport_area(
            self,
            Rect::new(0, 0, self.terminal_width, self.terminal_height),
        );
        let in_chat_area = mouse.column >= chat_area.x
            && mouse.column < chat_area.x + chat_area.width
            && mouse.row >= chat_area.y
            && mouse.row < chat_area.y + chat_area.height;

        if in_chat_area {
            match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_chat_up_lines(3),
                MouseEventKind::ScrollDown => self.scroll_chat_down_lines(3),
                _ => {}
            }
            self.clamp_chat_scroll();
        }
    }

    fn handle_permission_key(&mut self, key: KeyEvent) {
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

    async fn handle_slash_command(&mut self, user_tx: &mpsc::Sender<UserCommand>) {
        let cmd = self.input_text().trim().to_string();
        self.input_buffer.clear();
        self.reset_input_navigation();

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let name = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied();

        match name {
            "/clear" => {
                if matches!(arg, Some("keep-context") | Some("preserve-context")) {
                    self.messages.clear();
                    self.streaming_text.clear();
                    self.streaming_thinking.clear();
                    self.scroll_offset = 0;
                    self.follow_output = true;
                    self.messages.push(ChatMessage::System(
                        "Visible transcript cleared. Conversation context preserved in session state.".into(),
                    ));
                } else {
                    self.messages.clear();
                    self.streaming_text.clear();
                    self.streaming_thinking.clear();
                    self.scroll_offset = 0;
                    self.follow_output = true;
                    self.messages.push(ChatMessage::System("Session cleared.".into()));
                }
                self.sync_chat_viewport();
            }
            "/compact" => match user_tx.send(UserCommand::Compact).await {
                Ok(()) => {
                    self.messages.push(ChatMessage::System(
                        "Compacting conversation history...".into(),
                    ));
                    self.sync_chat_viewport();
                }
                Err(_) => {
                    self.messages.push(ChatMessage::System(
                        "Error: failed to send compact request".into(),
                    ));
                    self.sync_chat_viewport();
                }
            },
            "/mode" => {
                if let Some(mode_str) = arg {
                    match mode_str {
                        "default" | "accept-edits" | "bypass" | "plan" | "dont-ask" => {
                            match user_tx.send(UserCommand::SetMode(mode_str.to_string())).await {
                                Ok(()) => self.messages.push(ChatMessage::System(format!(
                                    "Switching permission mode to: {mode_str}"
                                ))),
                                Err(_) => self.messages.push(ChatMessage::System(
                                    "Error: failed to send mode change request".into(),
                                )),
                            }
                            self.sync_chat_viewport();
                        }
                        _ => {
                            self.messages.push(ChatMessage::System(
                                "Unknown mode. Valid modes: default, accept-edits, bypass, plan, dont-ask".into(),
                            ));
                            self.sync_chat_viewport();
                        }
                    }
                } else {
                    self.messages.push(ChatMessage::System(format!(
                        "Current mode: {}. Usage: /mode <default|accept-edits|bypass|plan|dont-ask>",
                        self.permission_mode
                    )));
                    self.sync_chat_viewport();
                }
            }
            "/model" => {
                if let Some(model_str) = arg {
                    match user_tx.send(UserCommand::SetModel(model_str.to_string())).await {
                        Ok(()) => self.messages.push(ChatMessage::System(format!(
                            "Switching model to: {model_str}"
                        ))),
                        Err(_) => self.messages.push(ChatMessage::System(
                            "Error: failed to send model change request".into(),
                        )),
                    }
                    self.sync_chat_viewport();
                } else {
                    self.messages.push(ChatMessage::System(format!(
                        "Current model setting: {}\nCurrent runtime model: {}",
                        self.model_setting, self.model
                    )));
                    self.sync_chat_viewport();
                }
            }
            "/theme" => {
                if let Some(theme_str) = arg {
                    let theme = match theme_str {
                        "dark" => Some(Theme::Dark),
                        "light" => Some(Theme::Light),
                        _ => None,
                    };
                    match theme {
                        Some(theme) => match user_tx.send(UserCommand::SetTheme(theme)).await {
                            Ok(()) => {
                                self.theme = theme;
                                self.messages.push(ChatMessage::System(format!(
                                    "Switching theme to: {theme_str}"
                                )));
                            }
                            Err(_) => self.messages.push(ChatMessage::System(
                                "Error: failed to send theme change request".into(),
                            )),
                        },
                        None => {
                            self.messages.push(ChatMessage::System(
                                "Unknown theme. Valid themes: dark, light".into(),
                            ));
                        }
                    }
                    self.sync_chat_viewport();
                } else {
                    self.messages.push(ChatMessage::System(format!(
                        "Current theme: {:?}. Usage: /theme <dark|light>",
                        self.theme
                    )));
                    self.sync_chat_viewport();
                }
            }
            "/memory" => {
                match arg {
                    None => {
                        let _ = user_tx.send(UserCommand::ShowMemory).await;
                    }
                    Some("remember") => {
                        if parts.len() < 6 {
                            self.messages.push(ChatMessage::System(
                                "Usage: /memory remember <type> <path> <title> <description> <body>".into(),
                            ));
                            self.sync_chat_viewport();
                        } else {
                            let memory_type = parts[2].to_string();
                            let path = parts[3].to_string();
                            let title = parts[4].to_string();
                            let description = parts[5].to_string();
                            let body = parts[6..].join(" ");
                            let _ = user_tx
                                .send(UserCommand::RememberMemory {
                                    memory_type,
                                    path,
                                    title,
                                    description,
                                    body,
                                })
                                .await;
                        }
                    }
                    Some("forget") => {
                        if parts.len() < 3 {
                            self.messages.push(ChatMessage::System(
                                "Usage: /memory forget <path>".into(),
                            ));
                            self.sync_chat_viewport();
                        } else {
                            let _ = user_tx
                                .send(UserCommand::ForgetMemory {
                                    path: parts[2].to_string(),
                                })
                                .await;
                        }
                    }
                    Some(_) => {
                        self.messages.push(ChatMessage::System(
                            "Usage: /memory [remember|forget] ...".into(),
                        ));
                        self.sync_chat_viewport();
                    }
                }
            }
            "/todo" => self.todo_visible = !self.todo_visible,
            "/config" => {
                let _ = user_tx.send(UserCommand::ShowConfig).await;
            }
            "/cost" => {
                let _ = user_tx.send(UserCommand::ShowCost).await;
            }
            "/diff" => {
                let _ = user_tx.send(UserCommand::ShowDiff).await;
            }
            "/hooks" => {
                let _ = user_tx.send(UserCommand::ShowHooks).await;
            }
            "/mcp" => {
                let _ = user_tx.send(UserCommand::ShowMcp).await;
            }
            "/help" => {
                self.messages
                    .push(ChatMessage::System(slash_command_help_text()));
                self.sync_chat_viewport();
            }
            "/exit" => self.should_quit = true,
            _ => {
                let detail = if has_slash_command(name) {
                    format!("Command parsing error: {}", name)
                } else {
                    format!("Unknown command: {}. Type /help for available commands.", name)
                };
                self.messages.push(ChatMessage::System(detail));
                self.sync_chat_viewport();
            }
        }
    }

    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::ThinkingStart => {
                self.is_thinking = true;
                self.streaming_thinking.clear();
                self.streaming_thinking_md.clear();
                self.thinking_folded = false;
                self.sync_chat_viewport();
            }
            AppEvent::ThinkingDelta(text) => {
                self.is_thinking = true;
                self.streaming_thinking.push_str(&text);
                self.streaming_thinking_md.push_delta(&text);
                self.sync_chat_viewport();
            }
            AppEvent::ThinkingComplete(text) => {
                self.is_thinking = false;
                let buffered = std::mem::take(&mut self.streaming_thinking);
                self.streaming_thinking_md.clear();
                let final_text = if text.is_empty() {
                    buffered
                } else {
                    text
                };
                if !final_text.is_empty() {
                    let summary = format!("Thought for ~{} chars", final_text.chars().count());
                    self.messages.push(ChatMessage::Thinking {
                        summary,
                        content: final_text,
                    });
                    self.update_selected_thinking();
                }
                self.sync_chat_viewport();
            }
            AppEvent::StreamStart => {
                self.is_streaming = true;
                self.suppress_stream = false;
                self.sync_chat_viewport();
            }
            AppEvent::StreamDelta(text) => {
                if self.is_streaming && !self.suppress_stream {
                    self.is_thinking = false;
                    self.streaming_text.push_str(&text);
                    self.streaming_text_raw.push_str(&text);
                    self.streaming_md.push_delta(&text);
                    self.sync_chat_viewport();
                }
            }
            AppEvent::StreamEnd => {
                // Finalize thinking if still buffered
                if !self.streaming_thinking.is_empty() {
                    let final_text = std::mem::take(&mut self.streaming_thinking);
                    self.streaming_thinking_md.clear();
                    let summary = format!("Thought for ~{} chars", final_text.chars().count());
                    self.messages.push(ChatMessage::Thinking {
                        summary,
                        content: final_text,
                    });
                    self.update_selected_thinking();
                }
                // Finalize streaming text
                if self.is_streaming && !self.suppress_stream && !self.streaming_text.is_empty() {
                    let text = std::mem::take(&mut self.streaming_text);
                    self.messages.push(ChatMessage::Assistant(text));
                } else {
                    self.streaming_text.clear();
                }
                // Clear all streaming state
                self.streaming_text_raw.clear();
                self.streaming_md.clear();
                self.streaming_tool = None;
                self.is_streaming = false;
                self.is_thinking = false;
                self.suppress_stream = false;
                self.sync_chat_viewport();
            }
            AppEvent::StreamCancelled => {
                // Finalize thinking as cancelled if any
                if !self.streaming_thinking.is_empty() {
                    let thinking_text = std::mem::take(&mut self.streaming_thinking);
                    self.streaming_thinking_md.clear();
                    let summary = format!("Thought for ~{} chars (cancelled)", thinking_text.chars().count());
                    self.messages.push(ChatMessage::Thinking {
                        summary,
                        content: thinking_text,
                    });
                    self.update_selected_thinking();
                }
                self.suppress_stream = true;
                self.is_streaming = false;
                self.is_thinking = false;
                self.streaming_text.clear();
                self.streaming_text_raw.clear();
                self.streaming_md.clear();
                self.streaming_thinking.clear();
                self.streaming_thinking_md.clear();
                self.streaming_tool = None;
                self.sync_chat_viewport();
            }
            AppEvent::ToolInputStreamStart { name } => {
                self.is_thinking = false;
                self.streaming_tool = Some(StreamingToolState {
                    name,
                    accumulated_json: String::new(),
                });
                self.sync_chat_viewport();
            }
            AppEvent::ToolInputDelta { name: _, json_fragment } => {
                if let Some(ref mut tool_state) = self.streaming_tool {
                    tool_state.accumulated_json.push_str(&json_fragment);
                    self.sync_chat_viewport();
                }
            }
            AppEvent::ToolUseStart { name, input } => {
                self.is_thinking = false;
                // Clear streaming tool state since the tool is now fully constructed
                self.streaming_tool = None;
                let summary = summarize_tool_input(&name, &input);
                self.messages.push(ChatMessage::ToolUse {
                    name,
                    input_summary: summary,
                });
                self.sync_chat_viewport();
            }
            AppEvent::ToolResult { name, output, is_error } => {
                let summary = truncate(&output, 200);
                self.messages.push(ChatMessage::ToolResult {
                    name,
                    output_summary: summary,
                    is_error,
                });
                self.sync_chat_viewport();
            }
            AppEvent::AssistantMessage(text) => {
                if !self.suppress_stream {
                    self.messages.push(ChatMessage::Assistant(text));
                }
                self.streaming_text.clear();
                self.streaming_thinking.clear();
                self.is_streaming = false;
                self.is_thinking = false;
                self.suppress_stream = false;
                self.sync_chat_viewport();
            }
            AppEvent::UsageUpdate {
                input_tokens,
                output_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
            } => {
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
                self.cache_read_input_tokens = cache_read_input_tokens;
                self.cache_creation_input_tokens = cache_creation_input_tokens;
            }
            AppEvent::StatusUpdate {
                model,
                model_setting,
                permission_mode,
                git_branch,
            } => {
                self.model = model;
                self.model_setting = model_setting;
                self.permission_mode = permission_mode;
                self.git_branch = git_branch;
            }
            AppEvent::ConfigInfo {
                model_source,
                permission_source,
                base_url_source,
                theme_source,
            } => {
                self.messages.push(ChatMessage::System(format!(
                    "Effective config:\n  model source: {}\n  permissions source: {}\n  base_url source: {}\n  theme source: {}",
                    model_source, permission_source, base_url_source, theme_source
                )));
                self.sync_chat_viewport();
            }
            AppEvent::Error(msg) => {
                self.messages.push(ChatMessage::System(msg));
                self.streaming_text.clear();
                self.streaming_thinking.clear();
                self.is_streaming = false;
                self.is_thinking = false;
                self.suppress_stream = false;
                self.sync_chat_viewport();
            }
            AppEvent::PermissionRequest {
                tool_name,
                input,
                response_tx,
            } => {
                let input_summary = summarize_tool_input(&tool_name, &input);
                self.permission_dialog = Some(PermissionDialog {
                    tool_name,
                    input_summary,
                    selected: 0,
                    response_tx: Some(response_tx),
                });
            }
            AppEvent::TodoUpdate(todos) => self.tasks = todos,
            AppEvent::CompactionStart => {
                self.messages.push(ChatMessage::System(
                    "Compacting conversation history...".into(),
                ));
                self.sync_chat_viewport();
            }
            AppEvent::CompactionComplete { result } => {
                self.messages.push(ChatMessage::System(format!(
                    "Compacted {} messages into summary. Preserved {} recent messages. Estimated tokens: {}K -> {}K",
                    result.compacted_message_count,
                    result.preserved_message_count,
                    result.estimated_tokens_before / 1000,
                    result.estimated_tokens_after / 1000,
                )));
                self.sync_chat_viewport();
            }
            AppEvent::HookBlocked { tool_name, reason } => {
                self.messages.push(ChatMessage::System(format!(
                    "Hook blocked {}: {}",
                    tool_name, reason,
                )));
                self.sync_chat_viewport();
            }
            AppEvent::Resize(w, h) => {
                self.terminal_width = w;
                self.terminal_height = h;
                self.clamp_chat_scroll();
            }
            AppEvent::Key(_) | AppEvent::Mouse(_) | AppEvent::Paste(_) => {}
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
        "Lsp" => {
            let op = input
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if path.is_empty() {
                op.to_string()
            } else {
                format!("{op} {path}")
            }
        }
        "WebFetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 100))
            .unwrap_or_default(),
        "WebSearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 100))
            .unwrap_or_default(),
        _ => truncate(&input.to_string(), 80),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }

    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    format!("{}...", &s[..end])
}

fn history_file_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("rust-claude-code").join("history");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("rust-claude-code")
            .join("history");
    }
    PathBuf::from(".rust-claude-history")
}

fn load_history(path: &PathBuf) -> Vec<String> {
    match fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .filter_map(|line| serde_json::from_str::<String>(line).ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn save_history(path: &PathBuf, history: &[String]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut serialized = String::new();
    for entry in history {
        serialized.push_str(&serde_json::to_string(entry).unwrap_or_else(|_| "\"\"".into()));
        serialized.push('\n');
    }
    fs::write(path, serialized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventKind, KeyEventState, MouseEvent, MouseEventKind};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_shift(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
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

    fn mouse(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn mouse_at(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_stream_start_reenables_streaming_after_tool_use_turn() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.handle_app_event(AppEvent::StreamEnd);
        assert!(!app.is_streaming);

        app.handle_app_event(AppEvent::StreamStart);
        app.handle_app_event(AppEvent::StreamDelta("final answer".into()));

        assert!(app.is_streaming);
        assert_eq!(app.streaming_text, "final answer");
    }

    #[test]
    fn test_help_registry_contains_new_commands() {
        let help = slash_command_help_text();
        assert!(help.contains("/config"));
        assert!(help.contains("/cost"));
        assert!(help.contains("/diff"));
        assert!(help.contains("/theme"));
    }

    #[test]
    fn test_input_buffer_multiline_insert_and_cursor() {
        let mut buffer = InputBuffer::new();
        buffer.insert_text("hello\nworld");
        assert_eq!(buffer.to_text(), "hello\nworld");
        assert_eq!(buffer.cursor(), CursorPosition { row: 1, col: 5 });
    }

    #[test]
    fn test_input_buffer_word_navigation() {
        let mut buffer = InputBuffer::from_text("hello rust world");
        buffer.set_cursor(0, 5);
        buffer.move_word_right();
        assert_eq!(buffer.cursor().col, 10);
        buffer.move_word_left();
        assert_eq!(buffer.cursor().col, 6);
    }

    #[tokio::test]
    async fn test_handle_paste_preserves_multiline_content() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.handle_paste("line1\r\nline2".into());
        assert_eq!(app.input_text(), "line1\nline2");
    }

    #[test]
    fn test_tab_toggles_selected_thinking_expansion() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages.push(ChatMessage::Thinking {
            summary: "Thought for ~10 chars".into(),
            content: "reasoning".into(),
        });
        app.update_selected_thinking();
        assert_eq!(app.selected_thinking, Some(0));
        assert!(app.expanded_thinking.is_empty());
        app.toggle_selected_thinking();
        assert_eq!(app.expanded_thinking, vec![0]);
        app.toggle_selected_thinking();
        assert!(app.expanded_thinking.is_empty());
    }

    #[tokio::test]
    async fn test_ctrl_c_cancels_stream_instead_of_quit() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.is_streaming = true;
        let (tx, mut rx) = mpsc::channel(1);
        app.handle_key_event(key_ctrl(KeyCode::Char('c')), &tx).await;
        assert!(!app.should_quit);
        assert!(!app.is_streaming);
        assert_eq!(rx.recv().await, Some(UserCommand::CancelStream));
    }

    #[tokio::test]
    async fn test_shift_enter_inserts_newline() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Char('a')), &tx).await;
        app.handle_key_event(key_shift(KeyCode::Enter), &tx).await;
        app.handle_key_event(key(KeyCode::Char('b')), &tx).await;
        assert_eq!(app.input_text(), "a\nb");
    }

    #[tokio::test]
    async fn test_up_down_browse_history() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.history = vec!["first".into(), "second".into()];
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Up), &tx).await;
        assert_eq!(app.input_text(), "second");
        app.handle_key_event(key(KeyCode::Up), &tx).await;
        assert_eq!(app.input_text(), "first");
        app.handle_key_event(key(KeyCode::Down), &tx).await;
        assert_eq!(app.input_text(), "second");
    }


    #[tokio::test]
    async fn test_page_navigation_preserves_draft() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.input_buffer = InputBuffer::from_text("draft");
        app.messages = (0..40)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        let original_cursor = app.input_cursor();
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::PageUp), &tx).await;

        assert_eq!(app.input_text(), "draft");
        assert_eq!(app.input_cursor(), original_cursor);
    }

    #[tokio::test]
    async fn test_page_down_restores_follow_output_at_bottom() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        app.follow_output = false;
        app.scroll_offset = 0;
        let (tx, _rx) = mpsc::channel(1);

        for _ in 0..20 {
            app.handle_key_event(key(KeyCode::PageDown), &tx).await;
        }

        assert!(app.follow_output);
        assert_eq!(app.scroll_offset, app.max_chat_scroll_offset());
    }

    #[tokio::test]
    async fn test_ctrl_home_and_end_jump_chat_boundaries() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key_ctrl(KeyCode::End), &tx).await;
        let latest = app.max_chat_scroll_offset();
        assert_eq!(app.scroll_offset, latest);

        app.handle_key_event(key_ctrl(KeyCode::Home), &tx).await;
        assert_eq!(app.scroll_offset, 0);
    }

    #[tokio::test]
    async fn test_page_up_from_follow_output_moves_up_from_latest() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        app.terminal_width = 80;
        app.terminal_height = 24;
        app.follow_output = true;
        app.scroll_offset = app.max_chat_scroll_offset();
        let initial = app.scroll_offset;
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::PageUp), &tx).await;

        assert!(app.scroll_offset < initial);
        assert!(!app.follow_output);
    }

    #[test]
    fn test_mouse_scroll_up_moves_away_from_latest() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        app.terminal_width = 80;
        app.terminal_height = 24;
        app.follow_output = true;
        app.scroll_offset = app.max_chat_scroll_offset();
        let initial = app.scroll_offset;

        app.handle_mouse_event(mouse(MouseEventKind::ScrollUp));

        assert!(app.scroll_offset < initial);
        assert!(!app.follow_output);
    }

    #[test]
    fn test_mouse_scroll_down_returns_to_latest() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        app.terminal_width = 80;
        app.terminal_height = 24;
        app.follow_output = false;
        app.scroll_offset = app.max_chat_scroll_offset().saturating_sub(6);

        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown));

        assert_eq!(app.scroll_offset, app.max_chat_scroll_offset());
        assert!(app.follow_output);
    }

    #[test]
    fn test_mouse_scroll_outside_chat_area_does_not_scroll_chat() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.todo_visible = true;
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        app.terminal_width = 120;
        app.terminal_height = 24;
        app.follow_output = true;
        app.scroll_offset = app.max_chat_scroll_offset();
        let initial = app.scroll_offset;

        app.handle_mouse_event(mouse_at(MouseEventKind::ScrollUp, 110, 2));

        assert_eq!(app.scroll_offset, initial);
        assert!(app.follow_output);
    }

    #[test]
    fn test_sync_chat_viewport_preserves_history_view() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.messages = (0..80)
            .map(|i| ChatMessage::Assistant(format!("message {i}")))
            .collect();
        app.follow_output = false;
        app.scroll_offset = 0;

        app.handle_app_event(AppEvent::AssistantMessage("new message".into()));

        assert_eq!(app.scroll_offset, 0);
        assert!(!app.follow_output);
    }
    #[tokio::test]
    async fn test_mode_command_sends_control_message() {
        let mut app = App::new("claude-sonnet-4-6".into(), "opusplan".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/mode plan");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::SetMode("plan".into()));
    }

    #[tokio::test]
    async fn test_model_command_sends_control_message() {
        let mut app = App::new("claude-sonnet-4-6".into(), "opusplan".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/model opus[1m]");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::SetModel("opus[1m]".into()));
    }

    #[tokio::test]
    async fn test_model_command_without_args_shows_setting_and_runtime() {
        let mut app = App::new("claude-opus-4-6".into(), "opusplan".into(), "Plan".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/model");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg))
                if msg.contains("Current model setting: opusplan")
                    && msg.contains("Current runtime model: claude-opus-4-6")
        ));
    }

    #[tokio::test]
    async fn test_theme_command_sends_control_message_and_updates_local_theme() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/theme light");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::SetTheme(Theme::Light));
        assert_eq!(app.theme, Theme::Light);
    }

    #[tokio::test]
    async fn test_memory_command_sends_show_memory() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/memory");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::ShowMemory);
    }

    #[tokio::test]
    async fn test_memory_remember_command_sends_payload() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text(
            "/memory remember feedback feedback/testing.md Testing Use-real-db Use real database",
        );
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(
            sent,
            UserCommand::RememberMemory {
                memory_type: "feedback".into(),
                path: "feedback/testing.md".into(),
                title: "Testing".into(),
                description: "Use-real-db".into(),
                body: "Use real database".into(),
            }
        );
    }

    #[tokio::test]
    async fn test_memory_forget_command_sends_payload() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/memory forget feedback/testing.md");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(
            sent,
            UserCommand::ForgetMemory {
                path: "feedback/testing.md".into()
            }
        );
    }

    #[tokio::test]
    async fn test_memory_remember_without_args_shows_usage() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/memory remember");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg))
                if msg.contains("Usage: /memory remember")
        ));
    }

    #[tokio::test]
    async fn test_memory_forget_without_args_shows_usage() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/memory forget");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg))
                if msg.contains("Usage: /memory forget")
        ));
    }

    #[tokio::test]
    async fn test_memory_unknown_subcommand_shows_usage() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/memory bogus");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg))
                if msg.contains("Usage: /memory [remember|forget]")
        ));
    }

    #[test]
    fn test_status_update_changes_displayed_model_and_mode() {
        let mut app = App::new("claude-sonnet-4-6".into(), "opusplan".into(), "Default".into(), None, Theme::Dark);
        app.handle_app_event(AppEvent::StatusUpdate {
            model: "claude-opus-4-6".into(),
            model_setting: "opusplan".into(),
            permission_mode: "Plan".into(),
            git_branch: Some("main".into()),
        });

        assert_eq!(app.model, "claude-opus-4-6");
        assert_eq!(app.model_setting, "opusplan");
        assert_eq!(app.permission_mode, "Plan");
        assert_eq!(app.git_branch.as_deref(), Some("main"));
    }

    #[test]
    fn test_thinking_complete_creates_message() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.handle_app_event(AppEvent::ThinkingComplete("reasoning".into()));
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::Thinking { summary, content })
                if summary.contains("Thought for") && content == "reasoning"
        ));
    }

    /// Verify that memory status text arriving as AssistantMessage is displayed
    /// correctly for the "no store" case.
    #[test]
    fn test_memory_no_store_response_displays_as_assistant() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let msg = "No memory store is available for the current project.";
        app.handle_app_event(AppEvent::AssistantMessage(msg.into()));

        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::Assistant(text)) if text.contains("No memory store")
        ));
    }

    /// Verify that memory status text for an empty store displays correctly.
    #[test]
    fn test_memory_empty_store_response_displays_as_assistant() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let msg = "Memory store:\n  project_root: /repo\n  memory_dir: /memory\n  entrypoint: /memory/MEMORY.md\n  entries: 0\n\nNo memory entries found.";
        app.handle_app_event(AppEvent::AssistantMessage(msg.into()));

        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::Assistant(text))
                if text.contains("entries: 0") && text.contains("No memory entries found")
        ));
    }

    /// Verify that memory status text for a populated store displays correctly.
    #[test]
    fn test_memory_populated_store_response_displays_as_assistant() {
        let mut app = App::new("test-model".into(), "test-model".into(), "Default".into(), None, Theme::Dark);
        let msg = "Memory store:\n  project_root: /repo\n  memory_dir: /memory\n  entrypoint: /memory/MEMORY.md\n  entries: 2\n\nVisible memories:\n- [feedback] testing.md — DB test guidance (1 days old)\n- [project] deploy.md — Deploy process (3 days old)\n";
        app.handle_app_event(AppEvent::AssistantMessage(msg.into()));

        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::Assistant(text))
                if text.contains("entries: 2") && text.contains("Visible memories:")
                    && text.contains("[feedback]") && text.contains("[project]")
        ));
    }

    #[test]
    fn test_cancel_during_text_streaming_clears_state() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.is_streaming = true;
        app.handle_app_event(AppEvent::StreamDelta("Hello ".into()));
        app.handle_app_event(AppEvent::StreamDelta("world".into()));
        assert!(!app.streaming_text.is_empty());
        assert!(!app.streaming_md.is_empty());

        // Cancel
        app.cancel_stream_local();
        assert!(app.streaming_text.is_empty());
        assert!(app.streaming_md.is_empty());
        assert!(app.streaming_text_raw.is_empty());
        assert!(!app.is_streaming);
        assert!(app.streaming_tool.is_none());
    }

    #[test]
    fn test_cancel_during_thinking_streaming_preserves_thinking() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.is_streaming = true;
        app.handle_app_event(AppEvent::ThinkingStart);
        app.handle_app_event(AppEvent::ThinkingDelta("Let me think about this...".into()));
        assert!(!app.streaming_thinking.is_empty());
        assert!(!app.streaming_thinking_md.is_empty());

        // Cancel
        app.cancel_stream_local();
        // Thinking should be finalized as a message with (cancelled)
        let thinking_msg = app.messages.iter().find(|m| matches!(m, ChatMessage::Thinking { .. }));
        assert!(thinking_msg.is_some(), "Thinking should be preserved as a message");
        if let Some(ChatMessage::Thinking { summary, content }) = thinking_msg {
            assert!(summary.contains("cancelled"), "Summary should note cancellation");
            assert_eq!(content, "Let me think about this...");
        }
        assert!(app.streaming_thinking.is_empty());
        assert!(app.streaming_thinking_md.is_empty());
        assert!(!app.is_streaming);
    }

    #[test]
    fn test_cancel_during_tool_input_streaming_clears_tool_state() {
        let mut app = App::new("test-model".into(), "test-model".into(), "default".into(), None, Theme::Dark);
        app.is_streaming = true;
        app.handle_app_event(AppEvent::ToolInputStreamStart { name: "Bash".into() });
        app.handle_app_event(AppEvent::ToolInputDelta {
            name: "Bash".into(),
            json_fragment: r#"{"command": "ls"#.into(),
        });
        assert!(app.streaming_tool.is_some());

        // Cancel
        app.cancel_stream_local();
        assert!(app.streaming_tool.is_none());
        assert!(!app.is_streaming);
    }

    #[test]
    fn test_truncate_handles_utf8_boundaries() {
        let input = "a".repeat(198) + "用例";
        let truncated = truncate(&input, 200);
        assert_eq!(truncated, format!("{}...", "a".repeat(198)));
    }

    #[test]
    fn test_truncate_does_not_split_multibyte_chars() {
        let input = "a".repeat(198) + "用例";
        let truncated = truncate(&input, 199);
        assert_eq!(truncated, format!("{}...", "a".repeat(198)));
    }
}

