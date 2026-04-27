use std::fs;
use std::io::Stdout;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use rust_claude_core::compaction::CompactStrategy;
use rust_claude_core::config::Theme;
use rust_claude_core::session::{ContextSnapshot, SessionSummary};
use rust_claude_core::state::TodoItem;
use rust_claude_tools::{AskUserQuestionRequest, AskUserQuestionResponse};
use tokio::sync::mpsc;

use crate::diff::{self, DiffLine};
use crate::events::{AppEvent, ChatMessage, PermissionResponse, UserCommand};
use crate::ui;

const CHAT_SCROLL_PAGE_SIZE: u16 = 8;

const MAX_HISTORY_ENTRIES: usize = 500;

struct SlashCommandSpec {
    name: &'static str,
    usage: &'static str,
    description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    Command,
    Skill,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionItem {
    pub kind: SuggestionKind,
    pub label: String,
    pub description: String,
    pub insert_text: String,
    match_text: String,
}

impl SuggestionItem {
    pub fn new(
        kind: SuggestionKind,
        label: impl Into<String>,
        description: impl Into<String>,
        insert_text: impl Into<String>,
        match_text: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            description: description.into(),
            insert_text: insert_text.into(),
            match_text: match_text.into(),
        }
    }

    fn matches_query(&self, query: &str) -> bool {
        self.match_text.contains(query)
    }

    fn match_rank(&self, query: &str) -> usize {
        if query.is_empty() {
            return 0;
        }
        let label = self.label.to_lowercase();
        if label == format!("/{query}") || label == query {
            0
        } else if label.starts_with(&format!("/{query}")) || label.starts_with(query) {
            1
        } else if self.match_text.contains(query) {
            2
        } else {
            3
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashSuggestions {
    pub query: String,
    pub items: Vec<SuggestionItem>,
    pub selected: usize,
    pub scroll: usize,
}

struct SkillSuggestionSpec {
    name: &'static str,
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
        usage: "/compact [default|aggressive|preserve-recent]",
        description: "Compact conversation history with an optional retention strategy",
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
        name: "/resume",
        usage: "/resume [session-id]",
        description: "Resume a saved session",
    },
    SlashCommandSpec {
        name: "/context",
        usage: "/context",
        description: "Show context window usage",
    },
    SlashCommandSpec {
        name: "/export",
        usage: "/export [path]",
        description: "Export conversation as Markdown",
    },
    SlashCommandSpec {
        name: "/copy",
        usage: "/copy",
        description: "Copy latest assistant response",
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
        usage: "/theme [dark|light|custom]",
        description: "Show or switch TUI theme",
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
        name: "/permissions",
        usage: "/permissions",
        description: "Show active permission rules",
    },
    SlashCommandSpec {
        name: "/init",
        usage: "/init",
        description: "Scaffold .claude/ directory with starter CLAUDE.md",
    },
    SlashCommandSpec {
        name: "/status",
        usage: "/status",
        description: "Show consolidated system overview",
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

const SKILL_SUGGESTIONS: &[SkillSuggestionSpec] = &[
    SkillSuggestionSpec {
        name: "brainstorming",
        description: "Explore requirements and design before implementation",
    },
    SkillSuggestionSpec {
        name: "test-driven-development",
        description: "Drive features and fixes with failing tests first",
    },
    SkillSuggestionSpec {
        name: "verification-before-completion",
        description: "Run fresh verification before claiming completion",
    },
    SkillSuggestionSpec {
        name: "openspec-propose",
        description: "Create a new OpenSpec change with proposal artifacts",
    },
    SkillSuggestionSpec {
        name: "openspec-apply-change",
        description: "Implement tasks from an existing OpenSpec change",
    },
];

fn slash_command_help_text() -> String {
    let mut text = String::from("Available commands:\n");
    for command in SLASH_COMMANDS {
        text.push_str(&format!(
            "  {:<22} — {}\n",
            command.usage, command.description
        ));
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
    /// Computed diff lines for FileEdit/FileWrite (None for other tools).
    pub diff_lines: Option<Vec<DiffLine>>,
    /// Current scroll offset in the diff preview area.
    pub diff_scroll: usize,
    /// Whether this is a file-modifying tool (FileEdit or FileWrite).
    pub is_file_tool: bool,
    /// File path being edited/written (for display in dialog header).
    pub file_path: Option<String>,
    /// Whether replace_all mode is active (FileEdit only).
    pub replace_all: bool,
}

/// State of the modal structured user-question dialog.
pub struct UserQuestionDialog {
    pub request: AskUserQuestionRequest,
    pub selected: usize,
    pub custom_input: InputBuffer,
    pub response_tx: Option<tokio::sync::oneshot::Sender<Option<AskUserQuestionResponse>>>,
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

#[derive(Debug, Clone)]
pub struct SessionPicker {
    pub sessions: Vec<SessionSummary>,
    pub selected: usize,
    pub scroll: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub skipped: usize,
}

impl SessionPicker {
    pub fn loading() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
            scroll: 0,
            loading: true,
            error: None,
            skipped: 0,
        }
    }

    fn with_sessions(sessions: Vec<SessionSummary>, skipped: usize) -> Self {
        Self {
            sessions,
            selected: 0,
            scroll: 0,
            loading: false,
            error: None,
            skipped,
        }
    }

    fn selected_session(&self) -> Option<&SessionSummary> {
        self.sessions.get(self.selected)
    }
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
    pub user_question_dialog: Option<UserQuestionDialog>,
    pub session_picker: Option<SessionPicker>,
    pub slash_suggestions: Option<SlashSuggestions>,

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
    pub custom_palette: Option<crate::theme::Palette>,
    pub active_theme_name: String,
}

impl App {
    pub fn palette(&self) -> crate::theme::Palette {
        self.custom_palette
            .unwrap_or_else(|| crate::theme::Palette::from_config(self.theme))
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
            user_question_dialog: None,
            session_picker: None,
            slash_suggestions: None,
            todo_visible: false,
            tasks: Vec::new(),
            history,
            history_index: None,
            draft_before_history: None,
            history_path,
            terminal_width: 80,
            terminal_height: 24,
            theme,
            custom_palette: None,
            active_theme_name: format!("{:?}", theme).to_lowercase(),
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

    fn refresh_slash_suggestions(&mut self) {
        let input = self.input_text();
        let Some(query) = input.strip_prefix('/') else {
            self.slash_suggestions = None;
            return;
        };

        let query = query.to_lowercase();
        let mut items = Vec::new();

        for command in SLASH_COMMANDS {
            let match_text = format!(
                "{} {} {}",
                command.name.to_lowercase(),
                command.usage.to_lowercase(),
                command.description.to_lowercase()
            );
            let item = SuggestionItem::new(
                SuggestionKind::Command,
                command.name,
                command.description,
                command.name,
                match_text,
            );
            if query.is_empty() || item.matches_query(&query) {
                items.push(item);
            }
        }

        for skill in SKILL_SUGGESTIONS {
            let match_text = format!(
                "{} {}",
                skill.name.to_lowercase(),
                skill.description.to_lowercase()
            );
            let item = SuggestionItem::new(
                SuggestionKind::Skill,
                skill.name,
                skill.description,
                skill.name,
                match_text,
            );
            if query.is_empty() || item.matches_query(&query) {
                items.push(item);
            }
        }

        items.sort_by(|a, b| {
            a.match_rank(&query)
                .cmp(&b.match_rank(&query))
                .then_with(|| a.label.cmp(&b.label))
        });

        self.slash_suggestions = Some(SlashSuggestions {
            query,
            items,
            selected: 0,
            scroll: 0,
        });
    }

    fn slash_suggestion_visible_rows(&self) -> usize {
        let input_height = self.input_buffer.line_count().max(1) as u16 + 2;
        let input_area = Rect::new(
            0,
            self.terminal_height.saturating_sub(1 + input_height),
            self.terminal_width,
            input_height,
        );
        crate::ui::slash_suggestion_overlay_geometry(self, input_area)
            .map(|(_, visible_rows)| visible_rows)
            .unwrap_or(1)
    }

    fn sync_suggestion_scroll_to_selection(&mut self) {
        let visible_rows = self.slash_suggestion_visible_rows();
        let Some(suggestions) = self.slash_suggestions.as_ref() else {
            return;
        };
        let (_, item_rows) = ui::build_slash_suggestion_render_rows(self, visible_rows.max(1));
        let Some(selected_row) = item_rows.get(suggestions.selected).copied() else {
            return;
        };
        let current_scroll = suggestions.scroll;
        let new_scroll = if selected_row < current_scroll {
            selected_row
        } else if selected_row >= current_scroll + visible_rows {
            selected_row + 1 - visible_rows
        } else {
            current_scroll
        };
        if let Some(suggestions) = self.slash_suggestions.as_mut() {
            suggestions.scroll = new_scroll;
        }
    }

    fn scroll_suggestions_up_page(&mut self) -> bool {
        let visible_rows = self.slash_suggestion_visible_rows();
        let Some(suggestions) = self.slash_suggestions.as_mut() else {
            return false;
        };
        if suggestions.items.is_empty() {
            return false;
        }
        suggestions.scroll = suggestions.scroll.saturating_sub(visible_rows);
        true
    }

    fn scroll_suggestions_down_page(&mut self) -> bool {
        let visible_rows = self.slash_suggestion_visible_rows();
        let Some(suggestions) = self.slash_suggestions.as_ref() else {
            return false;
        };
        if suggestions.items.is_empty() {
            return false;
        }
        let (rows, _) = ui::build_slash_suggestion_render_rows(self, visible_rows.max(1));
        let current_scroll = suggestions.scroll;
        let max_scroll = rows.len().saturating_sub(visible_rows);
        if let Some(suggestions) = self.slash_suggestions.as_mut() {
            suggestions.scroll = (current_scroll + visible_rows).min(max_scroll);
        }
        true
    }

    fn slash_suggestion_render_row_count(&self) -> usize {
        let (rows, _) = ui::build_slash_suggestion_render_rows(self, self.terminal_width as usize);
        rows.len()
    }

    fn move_suggestion_selection_up(&mut self) -> bool {
        let Some(suggestions) = self.slash_suggestions.as_mut() else {
            return false;
        };
        if suggestions.items.is_empty() {
            return false;
        }
        suggestions.selected = suggestions.selected.saturating_sub(1);
        self.sync_suggestion_scroll_to_selection();
        true
    }

    fn move_suggestion_selection_down(&mut self) -> bool {
        let Some(suggestions) = self.slash_suggestions.as_mut() else {
            return false;
        };
        if suggestions.items.is_empty() {
            return false;
        }
        suggestions.selected = (suggestions.selected + 1).min(suggestions.items.len() - 1);
        self.sync_suggestion_scroll_to_selection();
        true
    }

    fn apply_selected_suggestion(&mut self) -> bool {
        let Some(suggestions) = self.slash_suggestions.as_ref() else {
            return false;
        };
        let Some(selected) = suggestions.items.get(suggestions.selected) else {
            return false;
        };

        if self.input_text() == selected.insert_text {
            return false;
        }

        self.input_buffer = InputBuffer::from_text(&selected.insert_text);
        self.slash_suggestions = None;
        self.reset_input_navigation();
        true
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
            let summary = format!(
                "Thought for ~{} chars (cancelled)",
                thinking_text.chars().count()
            );
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
        self.messages
            .push(ChatMessage::System("Cancelled current response.".into()));
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
        self.refresh_slash_suggestions();
    }

    /// Process a keyboard event.
    pub async fn handle_key_event(&mut self, key: KeyEvent, user_tx: &mpsc::Sender<UserCommand>) {
        if self.user_question_dialog.is_some() {
            self.handle_user_question_key(key);
            return;
        }

        if self.permission_dialog.is_some() {
            self.handle_permission_key(key);
            return;
        }

        if self.session_picker.is_some() {
            self.handle_session_picker_key(key, user_tx).await;
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
            KeyCode::PageUp if !self.is_streaming && self.scroll_suggestions_up_page() => {}
            KeyCode::PageDown if !self.is_streaming && self.scroll_suggestions_down_page() => {}
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
                self.refresh_slash_suggestions();
            }
            KeyCode::Enter => {
                if self.is_streaming {
                    return;
                }
                self.submit_input(user_tx).await;
            }
            KeyCode::Char('e')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
            {
                self.input_buffer.move_end();
            }
            KeyCode::Char('a')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
            {
                self.input_buffer.move_home();
            }
            KeyCode::Char(' ') if !self.is_streaming => {
                self.input_buffer.insert_char(' ');
                self.reset_input_navigation();
                self.refresh_slash_suggestions();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
            {
                self.input_buffer.insert_char(c);
                self.reset_input_navigation();
                self.refresh_slash_suggestions();
            }
            KeyCode::Backspace if !self.is_streaming => {
                self.input_buffer.backspace();
                self.reset_input_navigation();
                self.refresh_slash_suggestions();
            }
            KeyCode::Delete if !self.is_streaming => {
                self.input_buffer.delete();
                self.reset_input_navigation();
                self.refresh_slash_suggestions();
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
            {
                self.input_buffer.move_word_left();
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.is_streaming =>
            {
                self.input_buffer.move_word_right();
            }
            KeyCode::Left if !self.is_streaming => self.input_buffer.move_left(),
            KeyCode::Right if !self.is_streaming => self.input_buffer.move_right(),
            KeyCode::Home if !self.is_streaming => self.input_buffer.move_home(),
            KeyCode::End if !self.is_streaming => self.input_buffer.move_end(),
            KeyCode::Up if !self.is_streaming && self.move_suggestion_selection_up() => {}
            KeyCode::Down if !self.is_streaming && self.move_suggestion_selection_down() => {}
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
                    self.slash_suggestions = None;
                    self.input_buffer.clear();
                    self.reset_input_navigation();
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if !self.is_streaming
                    && key.code == KeyCode::Tab
                    && self.apply_selected_suggestion()
                {
                } else if !self.is_streaming
                    && key.code == KeyCode::BackTab
                    && self.move_suggestion_selection_up()
                {
                } else if self.is_streaming && self.is_thinking {
                    // Toggle thinking fold/unfold during streaming
                    self.thinking_folded = !self.thinking_folded;
                } else if !self.is_streaming {
                    self.toggle_selected_thinking();
                }
            }
            KeyCode::Char('\t') => {
                if !self.is_streaming && self.apply_selected_suggestion() {
                } else if self.is_streaming && self.is_thinking {
                    self.thinking_folded = !self.thinking_folded;
                } else if !self.is_streaming {
                    self.toggle_selected_thinking();
                }
            }
            _ => {}
        }
        self.clamp_chat_scroll();
    }

    async fn handle_session_picker_key(
        &mut self,
        key: KeyEvent,
        user_tx: &mpsc::Sender<UserCommand>,
    ) {
        let page_size = self.session_picker_visible_rows();
        let picker = match self.session_picker.as_mut() {
            Some(picker) => picker,
            None => return,
        };

        match key.code {
            KeyCode::Esc => {
                self.session_picker = None;
            }
            KeyCode::Up => {
                picker.selected = picker.selected.saturating_sub(1);
                picker.scroll = picker.scroll.min(picker.selected);
            }
            KeyCode::Down => {
                if !picker.sessions.is_empty() {
                    picker.selected = (picker.selected + 1).min(picker.sessions.len() - 1);
                    if picker.selected >= picker.scroll + page_size {
                        picker.scroll = picker.selected + 1 - page_size;
                    }
                }
            }
            KeyCode::PageUp => {
                picker.selected = picker.selected.saturating_sub(page_size);
                picker.scroll = picker.scroll.saturating_sub(page_size).min(picker.selected);
            }
            KeyCode::PageDown => {
                if !picker.sessions.is_empty() {
                    picker.selected = (picker.selected + page_size).min(picker.sessions.len() - 1);
                    if picker.selected >= picker.scroll + page_size {
                        picker.scroll = picker.selected + 1 - page_size;
                    }
                }
            }
            KeyCode::Enter => {
                if picker.loading {
                    return;
                }
                if self.is_streaming {
                    self.messages.push(ChatMessage::System(
                        "Cannot resume another session while a response is active. Cancel or wait for it to finish first.".into(),
                    ));
                    self.sync_chat_viewport();
                    return;
                }
                if let Some(session) = picker.selected_session() {
                    let id = session.id.clone();
                    self.session_picker = None;
                    let _ = user_tx.send(UserCommand::ResumeSession(id)).await;
                }
            }
            _ => {}
        }
    }

    fn session_picker_visible_rows(&self) -> usize {
        ((self.terminal_height as usize * 60 / 100).saturating_sub(6)).max(3)
    }

    fn slash_suggestion_overlay_area(&self) -> Option<Rect> {
        let suggestions = self.slash_suggestions.as_ref()?;
        if suggestions.items.is_empty() {
            return None;
        }

        let input_height = self.input_buffer.line_count().max(1) as u16 + 2;
        let input_area = Rect::new(
            0,
            self.terminal_height.saturating_sub(1 + input_height),
            self.terminal_width,
            input_height,
        );
        crate::ui::slash_suggestion_overlay_geometry(self, input_area).map(|(area, _)| area)
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if let Some(overlay_area) = self.slash_suggestion_overlay_area() {
            let in_overlay = mouse.column >= overlay_area.x
                && mouse.column < overlay_area.x + overlay_area.width
                && mouse.row >= overlay_area.y
                && mouse.row < overlay_area.y + overlay_area.height;
            if in_overlay {
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        self.scroll_suggestions_up_page();
                    }
                    MouseEventKind::ScrollDown => {
                        self.scroll_suggestions_down_page();
                    }
                    _ => {}
                }
                return;
            }
        }

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

        match key.code {
            // Direct hotkeys always work
            KeyCode::Esc | KeyCode::Char('n') => {
                self.finish_permission_dialog(PermissionResponse::Deny)
            }
            KeyCode::Char('y') => self.finish_permission_dialog(PermissionResponse::Allow),
            KeyCode::Char('a') => self.finish_permission_dialog(PermissionResponse::AlwaysAllow),
            KeyCode::Char('d') => self.finish_permission_dialog(PermissionResponse::AlwaysDeny),
            // Up/Down: scroll diff if file tool, otherwise navigate options
            KeyCode::Up => {
                if dialog.is_file_tool && dialog.diff_lines.is_some() {
                    dialog.diff_scroll = dialog.diff_scroll.saturating_sub(1);
                } else {
                    dialog.selected = dialog.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if dialog.is_file_tool && dialog.diff_lines.is_some() {
                    // Clamp scroll conservatively to avoid overshooting the
                    // renderer's own viewport-aware cap in ui.rs.
                    let total_lines = dialog
                        .diff_lines
                        .as_ref()
                        .map(|d| d.len().saturating_add(1))
                        .unwrap_or(0);
                    let conservative_visible_lines = 8usize;
                    let max_scroll = total_lines.saturating_sub(conservative_visible_lines);
                    dialog.diff_scroll = (dialog.diff_scroll + 1).min(max_scroll);
                } else {
                    dialog.selected = (dialog.selected + 1).min(3);
                }
            }
            // PageUp/PageDown for faster diff scrolling
            KeyCode::PageUp => {
                if dialog.is_file_tool {
                    dialog.diff_scroll = dialog.diff_scroll.saturating_sub(10);
                }
            }
            KeyCode::PageDown => {
                if dialog.is_file_tool {
                    let total_lines = dialog
                        .diff_lines
                        .as_ref()
                        .map(|d| d.len().saturating_add(1))
                        .unwrap_or(0);
                    let conservative_visible_lines = 8usize;
                    let max_scroll = total_lines.saturating_sub(conservative_visible_lines);
                    dialog.diff_scroll = (dialog.diff_scroll + 10).min(max_scroll);
                }
            }
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

    fn handle_user_question_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.finish_user_question_dialog(None);
            return;
        }

        if key.code == KeyCode::Enter {
            let response = self.user_question_dialog.as_ref().and_then(|dialog| {
                let option_count = dialog.request.options.len();
                let custom_selected =
                    dialog.request.allow_custom && dialog.selected == option_count;
                if custom_selected {
                    let answer = dialog.custom_input.to_text().trim().to_string();
                    if answer.is_empty() {
                        return None;
                    }
                    Some(AskUserQuestionResponse {
                        selected_label: None,
                        answer,
                        custom: true,
                    })
                } else {
                    dialog.request.options.get(dialog.selected).map(|option| {
                        let answer = if option.description.trim().is_empty() {
                            option.label.clone()
                        } else {
                            option.description.clone()
                        };
                        AskUserQuestionResponse {
                            selected_label: Some(option.label.clone()),
                            answer,
                            custom: false,
                        }
                    })
                }
            });
            if let Some(response) = response {
                self.finish_user_question_dialog(Some(response));
            }
            return;
        }

        let dialog = match self.user_question_dialog.as_mut() {
            Some(d) => d,
            None => return,
        };
        let option_count = dialog.request.options.len();
        let max_selected = if dialog.request.allow_custom {
            option_count
        } else {
            option_count.saturating_sub(1)
        };
        let custom_selected = dialog.request.allow_custom && dialog.selected == option_count;

        match key.code {
            KeyCode::Up => dialog.selected = dialog.selected.saturating_sub(1),
            KeyCode::Down => dialog.selected = (dialog.selected + 1).min(max_selected),
            KeyCode::Char(c)
                if custom_selected && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                dialog.custom_input.insert_char(c);
            }
            KeyCode::Backspace if custom_selected => dialog.custom_input.backspace(),
            KeyCode::Delete if custom_selected => dialog.custom_input.delete(),
            KeyCode::Left if custom_selected => dialog.custom_input.move_left(),
            KeyCode::Right if custom_selected => dialog.custom_input.move_right(),
            KeyCode::Home if custom_selected => dialog.custom_input.move_home(),
            KeyCode::End if custom_selected => dialog.custom_input.move_end(),
            _ => {}
        }
    }

    fn finish_user_question_dialog(&mut self, response: Option<AskUserQuestionResponse>) {
        if let Some(dialog) = self.user_question_dialog.take() {
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
                    self.messages
                        .push(ChatMessage::System("Session cleared.".into()));
                }
                self.sync_chat_viewport();
            }
            "/compact" => {
                let strategy = match arg {
                    None => CompactStrategy::Default,
                    Some(value) if parts.len() == 2 => match value.parse::<CompactStrategy>() {
                        Ok(strategy) => strategy,
                        Err(error) => {
                            self.messages.push(ChatMessage::System(error));
                            self.sync_chat_viewport();
                            return;
                        }
                    },
                    Some(_) => {
                        self.messages.push(ChatMessage::System(
                            "Usage: /compact [default|aggressive|preserve-recent]".into(),
                        ));
                        self.sync_chat_viewport();
                        return;
                    }
                };

                match user_tx.send(UserCommand::Compact(strategy)).await {
                    Ok(()) => {
                        self.messages.push(ChatMessage::System(format!(
                            "Compacting conversation history with {} strategy...",
                            strategy.as_str()
                        )));
                        self.sync_chat_viewport();
                    }
                    Err(_) => {
                        self.messages.push(ChatMessage::System(
                            "Error: failed to send compact request".into(),
                        ));
                        self.sync_chat_viewport();
                    }
                }
            }
            "/mode" => {
                if let Some(mode_str) = arg {
                    match mode_str {
                        "default" | "accept-edits" | "bypass" | "plan" | "dont-ask" => {
                            match user_tx
                                .send(UserCommand::SetMode(mode_str.to_string()))
                                .await
                            {
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
                    match user_tx
                        .send(UserCommand::SetModel(model_str.to_string()))
                        .await
                    {
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
                                self.custom_palette = None;
                                self.active_theme_name = theme_str.to_string();
                                self.messages.push(ChatMessage::System(format!(
                                    "Switching theme to: {theme_str}"
                                )));
                            }
                            Err(_) => self.messages.push(ChatMessage::System(
                                "Error: failed to send theme change request".into(),
                            )),
                        },
                        None => {
                            if theme_str == "custom" {
                                match crate::theme::load_custom_palette_default() {
                                    Ok(palette) => {
                                        self.custom_palette = Some(palette);
                                        self.active_theme_name = "custom".into();
                                        self.messages.push(ChatMessage::System(
                                            "Custom theme loaded from ~/.config/rust-claude-code/theme.json".into(),
                                        ));
                                    }
                                    Err(error) => {
                                        self.messages.push(ChatMessage::System(format!(
                                            "Failed to load custom theme: {error}"
                                        )));
                                    }
                                }
                            } else {
                                self.messages.push(ChatMessage::System(
                                    "Unknown theme. Valid themes: dark, light, custom".into(),
                                ));
                            }
                        }
                    }
                    self.sync_chat_viewport();
                } else {
                    self.messages.push(ChatMessage::System(format!(
                        "Current theme: {}. Available themes: dark, light, custom. Usage: /theme [dark|light|custom]",
                        self.active_theme_name
                    )));
                    self.sync_chat_viewport();
                }
            }
            "/memory" => match arg {
                None => {
                    let _ = user_tx.send(UserCommand::ShowMemory).await;
                }
                Some("remember") => {
                    if parts.len() < 6 {
                        self.messages.push(ChatMessage::System(
                            "Usage: /memory remember <type> <path> <title> <description> <body>"
                                .into(),
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
                        self.messages
                            .push(ChatMessage::System("Usage: /memory forget <path>".into()));
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
            },
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
            "/resume" => {
                if self.is_streaming {
                    self.messages.push(ChatMessage::System(
                        "Cannot resume another session while a response is active. Cancel or wait for it to finish first.".into(),
                    ));
                    self.sync_chat_viewport();
                } else if let Some(session_id) = arg {
                    let _ = user_tx
                        .send(UserCommand::ResumeSession(session_id.to_string()))
                        .await;
                } else {
                    self.session_picker = Some(SessionPicker::loading());
                    let _ = user_tx.send(UserCommand::ListSessions).await;
                }
            }
            "/context" => {
                let _ = user_tx.send(UserCommand::ShowContext).await;
            }
            "/export" => {
                let path = cmd
                    .strip_prefix("/export")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from);
                let _ = user_tx.send(UserCommand::ExportConversation { path }).await;
            }
            "/copy" => {
                let _ = user_tx.send(UserCommand::CopyLatestAssistant).await;
                if self.is_streaming {
                    self.messages.push(ChatMessage::System(
                        "Active streaming response is not yet copyable; copying the previous completed assistant response if available.".into(),
                    ));
                    self.sync_chat_viewport();
                }
            }
            "/hooks" => {
                let _ = user_tx.send(UserCommand::ShowHooks).await;
            }
            "/mcp" => {
                let _ = user_tx.send(UserCommand::ShowMcp).await;
            }
            "/permissions" => {
                let _ = user_tx.send(UserCommand::ShowPermissions).await;
            }
            "/init" => {
                let _ = user_tx.send(UserCommand::InitProject).await;
            }
            "/status" => {
                let _ = user_tx.send(UserCommand::ShowStatus).await;
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
                    format!(
                        "Unknown command: {}. Type /help for available commands.",
                        name
                    )
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
                let final_text = if text.is_empty() { buffered } else { text };
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
                    let cleaned = crate::ui::strip_ansi_codes(&text);
                    self.streaming_text.push_str(&cleaned);
                    self.streaming_text_raw.push_str(&text);
                    self.streaming_md.push_delta(&cleaned);
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
                    let summary = format!(
                        "Thought for ~{} chars (cancelled)",
                        thinking_text.chars().count()
                    );
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
            AppEvent::ToolInputDelta {
                name: _,
                json_fragment,
            } => {
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
                let (tool_diff_lines, _, _, _) = extract_diff_info(&name, &input);
                self.messages.push(ChatMessage::ToolUse {
                    name,
                    input_summary: summary,
                    diff_lines: tool_diff_lines,
                });
                self.sync_chat_viewport();
            }
            AppEvent::ToolResult {
                name,
                output,
                is_error,
            } => {
                let cleaned = crate::ui::strip_ansi_codes(&output);
                let summary = summarize_tool_result(&name, &cleaned);
                self.messages.push(ChatMessage::ToolResult {
                    name,
                    output_summary: summary,
                    is_error,
                });
                self.sync_chat_viewport();
            }
            AppEvent::AssistantMessage(text) => {
                if !self.suppress_stream {
                    let cleaned = crate::ui::strip_ansi_codes(&text);
                    self.messages.push(ChatMessage::Assistant(cleaned));
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
            AppEvent::SessionList { sessions, skipped } => {
                self.session_picker = Some(SessionPicker::with_sessions(sessions, skipped));
            }
            AppEvent::SessionResumed {
                summary,
                messages,
                model,
                model_setting,
                permission_mode,
                git_branch,
                input_tokens,
                output_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
            } => {
                self.messages = messages;
                self.messages.push(ChatMessage::System(format!(
                    "Resumed session {} ({} messages)",
                    summary.id, summary.message_count
                )));
                self.model = model;
                self.model_setting = model_setting;
                self.permission_mode = permission_mode;
                self.git_branch = git_branch;
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
                self.cache_read_input_tokens = cache_read_input_tokens;
                self.cache_creation_input_tokens = cache_creation_input_tokens;
                self.streaming_text.clear();
                self.streaming_text_raw.clear();
                self.streaming_thinking.clear();
                self.streaming_tool = None;
                self.is_streaming = false;
                self.is_thinking = false;
                self.session_picker = None;
                self.jump_chat_to_latest();
            }
            AppEvent::ContextSnapshot(snapshot) => {
                self.messages
                    .push(ChatMessage::System(format_context_snapshot(
                        &snapshot,
                        self.terminal_width,
                    )));
                self.sync_chat_viewport();
            }
            AppEvent::Error(msg) => {
                self.messages.push(ChatMessage::System(msg));
                if let Some(picker) = self.session_picker.as_mut() {
                    picker.loading = false;
                    picker.error = Some(
                        self.messages
                            .last()
                            .map(ChatMessage::body)
                            .unwrap_or("")
                            .to_string(),
                    );
                }
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
                let (diff_lines, is_file_tool, file_path, replace_all) =
                    extract_diff_info(&tool_name, &input);
                self.permission_dialog = Some(PermissionDialog {
                    tool_name,
                    input_summary,
                    selected: 0,
                    response_tx: Some(response_tx),
                    diff_lines,
                    diff_scroll: 0,
                    is_file_tool,
                    file_path,
                    replace_all,
                });
            }
            AppEvent::UserQuestionRequest {
                request,
                response_tx,
            } => {
                self.user_question_dialog = Some(UserQuestionDialog {
                    request,
                    selected: 0,
                    custom_input: InputBuffer::new(),
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
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
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
        "AskUserQuestion" => {
            let question = input.get("question").and_then(|v| v.as_str()).unwrap_or("");
            truncate(question, 100)
        }
        "TodoWrite" => {
            let count = input
                .get("todos")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if count == 0 {
                "update tasks".to_string()
            } else {
                format!("{count} tasks")
            }
        }
        _ => {
            // For unknown tools, try to extract a meaningful field rather than
            // dumping the entire JSON object.
            let candidates = ["query", "question", "path", "url", "command", "name"];
            for key in &candidates {
                if let Some(val) = input.get(*key).and_then(|v| v.as_str()) {
                    return truncate(val, 80);
                }
            }
            // Fallback: truncated JSON, but clean it up
            let json_str = input.to_string();
            if json_str.len() <= 80 {
                json_str
            } else {
                truncate(&json_str, 60)
            }
        }
    }
}

/// Produce a human-readable summary of a tool result.
///
/// For tools whose output is structured JSON (e.g. AskUserQuestion), this
/// extracts key fields so users see a short, readable string instead of raw
/// JSON.
fn summarize_tool_result(tool_name: &str, output: &str) -> String {
    match tool_name {
        "AskUserQuestion" => {
            // The result is JSON like {"selected_label":"X","answer":"Y","custom":false}
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(output) {
                let label = val
                    .get("selected_label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let answer = val.get("answer").and_then(|v| v.as_str()).unwrap_or("");
                let custom = val.get("custom").and_then(|v| v.as_bool()).unwrap_or(false);
                if custom {
                    truncate(answer, 200)
                } else if !label.is_empty() {
                    label.to_string()
                } else {
                    truncate(answer, 200)
                }
            } else {
                truncate(output, 200)
            }
        }
        _ => truncate(output, 200),
    }
}

fn format_context_snapshot(snapshot: &ContextSnapshot, terminal_width: u16) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Context usage ({})", snapshot.model));
    match snapshot.context_capacity {
        Some(capacity) => {
            let pct = if capacity == 0 {
                0.0
            } else {
                snapshot.used_tokens as f64 * 100.0 / capacity as f64
            };
            lines.push(format!(
                "  used: {} / {} tokens ({:.1}%)",
                snapshot.used_tokens, capacity, pct
            ));
            lines.push(format!(
                "  remaining: {} tokens",
                snapshot.remaining_tokens.unwrap_or(0)
            ));
            if terminal_width >= 72 {
                lines.push(format!(
                    "  [{}]",
                    context_bar(
                        snapshot.system_prompt_tokens,
                        snapshot.message_tokens,
                        snapshot.tool_result_tokens,
                        snapshot.remaining_tokens.unwrap_or(0),
                        capacity,
                        40,
                    )
                ));
                lines.push("  S=system M=messages T=tool results .=remaining".into());
            }
        }
        None => {
            lines.push(format!("  used: {} tokens", snapshot.used_tokens));
            lines.push("  remaining: unavailable (unknown model context capacity)".into());
        }
    }
    lines.push(format!(
        "  system prompt: {} tokens",
        snapshot.system_prompt_tokens
    ));
    lines.push(format!("  messages: {} tokens", snapshot.message_tokens));
    lines.push(format!(
        "  tool results: {} tokens",
        snapshot.tool_result_tokens
    ));
    lines.join("\n")
}

fn context_bar(
    system: u32,
    messages: u32,
    tools: u32,
    remaining: u32,
    capacity: u32,
    width: usize,
) -> String {
    if capacity == 0 || width == 0 {
        return String::new();
    }
    let segments = [
        ('S', system),
        ('M', messages),
        ('T', tools),
        ('.', remaining),
    ];
    let mut bar = String::new();
    for (ch, tokens) in segments {
        let count = ((tokens as f64 / capacity as f64) * width as f64).round() as usize;
        bar.push_str(&ch.to_string().repeat(count));
    }
    if bar.len() < width {
        bar.push_str(&".".repeat(width - bar.len()));
    }
    bar.chars().take(width).collect()
}

/// Extract diff information from a tool's input for the permission dialog.
/// Returns (diff_lines, is_file_tool, file_path, replace_all).
fn extract_diff_info(
    tool_name: &str,
    input: &serde_json::Value,
) -> (Option<Vec<DiffLine>>, bool, Option<String>, bool) {
    match tool_name {
        "FileEdit" => {
            let file_path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let old_string = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_string = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let replace_all = input
                .get("replace_all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let diff_lines = diff::compute_diff(old_string, new_string);
            (Some(diff_lines), true, file_path, replace_all)
        }
        "FileWrite" => {
            let file_path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            // For FileWrite, show all content as additions (new file)
            let diff_lines = diff::compute_diff("", content);
            (Some(diff_lines), true, file_path, false)
        }
        _ => (None, false, None, false),
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        assert!(help.contains("/resume [session-id]"));
        assert!(help.contains("/context"));
        assert!(help.contains("/export [path]"));
        assert!(help.contains("/copy"));
        assert!(help.contains("/theme [dark|light|custom]"));
        assert!(help.contains("/compact [default|aggressive|preserve-recent]"));
        assert!(has_slash_command("/resume"));
        assert!(has_slash_command("/context"));
        assert!(has_slash_command("/export"));
        assert!(has_slash_command("/copy"));
    }

    #[tokio::test]
    async fn test_compact_command_without_arg_sends_default_strategy() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/compact");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert_eq!(
            rx.recv().await,
            Some(UserCommand::Compact(CompactStrategy::Default))
        );
    }

    #[tokio::test]
    async fn test_compact_command_with_named_strategy() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/compact preserve-recent");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert_eq!(
            rx.recv().await,
            Some(UserCommand::Compact(CompactStrategy::PreserveRecent))
        );
    }

    #[tokio::test]
    async fn test_compact_command_with_unknown_strategy_shows_error() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/compact tiny");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(message)) if message.contains("unknown compact strategy")
        ));
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.handle_paste("line1\r\nline2".into());
        assert_eq!(app.input_text(), "line1\nline2");
    }

    #[tokio::test]
    async fn test_user_question_selects_option() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        app.handle_app_event(AppEvent::UserQuestionRequest {
            request: AskUserQuestionRequest {
                question: "Pick a mode".into(),
                options: vec![
                    rust_claude_tools::AskUserQuestionOption {
                        label: "Fast".into(),
                        description: "Use fast path".into(),
                    },
                    rust_claude_tools::AskUserQuestionOption {
                        label: "Careful".into(),
                        description: "Use careful path".into(),
                    },
                ],
                allow_custom: false,
            },
            response_tx,
        });
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Down), &tx).await;
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let response = response_rx.await.unwrap().unwrap();
        assert_eq!(response.selected_label.as_deref(), Some("Careful"));
        assert_eq!(response.answer, "Use careful path");
        assert!(!response.custom);
        assert!(app.user_question_dialog.is_none());
    }

    #[tokio::test]
    async fn test_user_question_custom_answer() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        app.handle_app_event(AppEvent::UserQuestionRequest {
            request: AskUserQuestionRequest {
                question: "Pick a mode".into(),
                options: vec![rust_claude_tools::AskUserQuestionOption {
                    label: "Fast".into(),
                    description: "Use fast path".into(),
                }],
                allow_custom: true,
            },
            response_tx,
        });
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Down), &tx).await;
        for ch in "custom".chars() {
            app.handle_key_event(key(KeyCode::Char(ch)), &tx).await;
        }
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let response = response_rx.await.unwrap().unwrap();
        assert_eq!(response.selected_label, None);
        assert_eq!(response.answer, "custom");
        assert!(response.custom);
    }

    #[tokio::test]
    async fn test_user_question_cancel_sends_none() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        app.handle_app_event(AppEvent::UserQuestionRequest {
            request: AskUserQuestionRequest {
                question: "Pick a mode".into(),
                options: vec![],
                allow_custom: true,
            },
            response_tx,
        });
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Esc), &tx).await;

        assert_eq!(response_rx.await.unwrap(), None);
        assert!(app.user_question_dialog.is_none());
    }

    #[test]
    fn test_tab_toggles_selected_thinking_expansion() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.is_streaming = true;
        let (tx, mut rx) = mpsc::channel(1);
        app.handle_key_event(key_ctrl(KeyCode::Char('c')), &tx)
            .await;
        assert!(!app.should_quit);
        assert!(!app.is_streaming);
        assert_eq!(rx.recv().await, Some(UserCommand::CancelStream));
    }

    #[tokio::test]
    async fn test_shift_enter_inserts_newline() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Char('a')), &tx).await;
        app.handle_key_event(key_shift(KeyCode::Enter), &tx).await;
        app.handle_key_event(key(KeyCode::Char('b')), &tx).await;
        assert_eq!(app.input_text(), "a\nb");
    }

    #[tokio::test]
    async fn test_up_down_browse_history() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "claude-sonnet-4-6".into(),
            "opusplan".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/mode plan");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::SetMode("plan".into()));
    }

    #[tokio::test]
    async fn test_model_command_sends_control_message() {
        let mut app = App::new(
            "claude-sonnet-4-6".into(),
            "opusplan".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/model opus[1m]");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::SetModel("opus[1m]".into()));
    }

    #[tokio::test]
    async fn test_model_command_without_args_shows_setting_and_runtime() {
        let mut app = App::new(
            "claude-opus-4-6".into(),
            "opusplan".into(),
            "Plan".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/theme light");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::SetTheme(Theme::Light));
        assert_eq!(app.theme, Theme::Light);
        assert_eq!(app.active_theme_name, "light");
    }

    #[tokio::test]
    async fn test_theme_without_args_lists_themes() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/theme");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg))
                if msg.contains("Available themes: dark, light, custom")
                    && msg.contains("/theme [dark|light|custom]")
        ));
    }

    #[tokio::test]
    async fn test_resume_without_args_opens_picker_and_requests_sessions() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/resume");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(app
            .session_picker
            .as_ref()
            .is_some_and(|picker| picker.loading));
        assert_eq!(rx.recv().await, Some(UserCommand::ListSessions));
    }

    #[tokio::test]
    async fn test_resume_with_id_sends_resume_command() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/resume 20260426_120000");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert_eq!(
            rx.recv().await,
            Some(UserCommand::ResumeSession("20260426_120000".into()))
        );
    }

    #[tokio::test]
    async fn test_resume_blocks_while_streaming() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        app.is_streaming = true;
        app.session_picker = Some(SessionPicker::with_sessions(
            vec![SessionSummary {
                id: "one".into(),
                model: "claude".into(),
                model_setting: "sonnet".into(),
                cwd: "/tmp".into(),
                created_at: "2026-04-26T10:00:00+08:00".into(),
                updated_at: "2026-04-26T10:00:00+08:00".into(),
                message_count: 1,
                first_user_summary: "first".into(),
                total_usage: None,
            }],
            0,
        ));
        let (tx, mut rx) = mpsc::channel(1);
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(app.session_picker.is_some());
        assert!(rx.try_recv().is_err());
        assert!(matches!(
            app.messages.last(),
            Some(ChatMessage::System(msg)) if msg.contains("Cannot resume another session")
        ));
    }

    #[test]
    fn test_session_list_empty_and_error_states() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        app.session_picker = Some(SessionPicker::loading());

        app.handle_app_event(AppEvent::SessionList {
            sessions: Vec::new(),
            skipped: 0,
        });
        let picker = app.session_picker.as_ref().unwrap();
        assert!(!picker.loading);
        assert!(picker.sessions.is_empty());

        app.handle_app_event(AppEvent::Error("Session 'missing' not found".into()));
        assert!(app
            .session_picker
            .as_ref()
            .and_then(|picker| picker.error.as_ref())
            .is_some_and(|error| error.contains("missing")));
    }

    #[tokio::test]
    async fn test_context_export_and_copy_commands_dispatch() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(3);

        app.input_buffer = InputBuffer::from_text("/context");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;
        assert_eq!(rx.recv().await, Some(UserCommand::ShowContext));

        app.input_buffer = InputBuffer::from_text("/export /tmp/session export.md");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;
        assert_eq!(
            rx.recv().await,
            Some(UserCommand::ExportConversation {
                path: Some(PathBuf::from("/tmp/session export.md"))
            })
        );

        app.input_buffer = InputBuffer::from_text("/copy");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;
        assert_eq!(rx.recv().await, Some(UserCommand::CopyLatestAssistant));
    }

    #[tokio::test]
    async fn test_session_picker_navigation_and_confirm() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        app.session_picker = Some(SessionPicker::with_sessions(
            vec![
                SessionSummary {
                    id: "one".into(),
                    model: "claude".into(),
                    model_setting: "sonnet".into(),
                    cwd: "/tmp".into(),
                    created_at: "2026-04-26T10:00:00+08:00".into(),
                    updated_at: "2026-04-26T10:00:00+08:00".into(),
                    message_count: 1,
                    first_user_summary: "first".into(),
                    total_usage: None,
                },
                SessionSummary {
                    id: "two".into(),
                    model: "claude".into(),
                    model_setting: "opus".into(),
                    cwd: "/tmp".into(),
                    created_at: "2026-04-26T11:00:00+08:00".into(),
                    updated_at: "2026-04-26T11:00:00+08:00".into(),
                    message_count: 2,
                    first_user_summary: "second".into(),
                    total_usage: None,
                },
            ],
            0,
        ));
        let (tx, mut rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Down), &tx).await;
        assert_eq!(app.session_picker.as_ref().unwrap().selected, 1);
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert_eq!(
            rx.recv().await,
            Some(UserCommand::ResumeSession("two".into()))
        );
        assert!(app.session_picker.is_none());
    }

    #[tokio::test]
    async fn test_session_picker_cancel_preserves_input() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        app.input_buffer = InputBuffer::from_text("draft");
        app.session_picker = Some(SessionPicker::loading());
        let (tx, mut rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Esc), &tx).await;

        assert!(app.session_picker.is_none());
        assert_eq!(app.input_text(), "draft");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_memory_command_sends_show_memory() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);
        app.input_buffer = InputBuffer::from_text("/memory");
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let sent = rx.recv().await.unwrap();
        assert_eq!(sent, UserCommand::ShowMemory);
    }

    #[tokio::test]
    async fn test_memory_remember_command_sends_payload() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "claude-sonnet-4-6".into(),
            "opusplan".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "Default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
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
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.is_streaming = true;
        app.handle_app_event(AppEvent::ThinkingStart);
        app.handle_app_event(AppEvent::ThinkingDelta("Let me think about this...".into()));
        assert!(!app.streaming_thinking.is_empty());
        assert!(!app.streaming_thinking_md.is_empty());

        // Cancel
        app.cancel_stream_local();
        // Thinking should be finalized as a message with (cancelled)
        let thinking_msg = app
            .messages
            .iter()
            .find(|m| matches!(m, ChatMessage::Thinking { .. }));
        assert!(
            thinking_msg.is_some(),
            "Thinking should be preserved as a message"
        );
        if let Some(ChatMessage::Thinking { summary, content }) = thinking_msg {
            assert!(
                summary.contains("cancelled"),
                "Summary should note cancellation"
            );
            assert_eq!(content, "Let me think about this...");
        }
        assert!(app.streaming_thinking.is_empty());
        assert!(app.streaming_thinking_md.is_empty());
        assert!(!app.is_streaming);
    }

    #[test]
    fn test_cancel_during_tool_input_streaming_clears_tool_state() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.is_streaming = true;
        app.handle_app_event(AppEvent::ToolInputStreamStart {
            name: "Bash".into(),
        });
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
    #[test]
    fn test_extract_diff_info_file_edit() {
        let input = serde_json::json!({
            "file_path": "src/main.rs",
            "old_string": "hello",
            "new_string": "world",
            "replace_all": false
        });
        let (diff_lines, is_file_tool, file_path, replace_all) =
            extract_diff_info("FileEdit", &input);
        assert!(is_file_tool);
        assert!(diff_lines.is_some());
        assert_eq!(file_path.as_deref(), Some("src/main.rs"));
        assert!(!replace_all);
        let diff = diff_lines.unwrap();
        assert!(!diff.is_empty());
    }

    #[test]
    fn test_extract_diff_info_file_edit_replace_all() {
        let input = serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "foo",
            "new_string": "bar",
            "replace_all": true
        });
        let (_, _, _, replace_all) = extract_diff_info("FileEdit", &input);
        assert!(replace_all);
    }

    #[test]
    fn test_extract_diff_info_file_write() {
        let input = serde_json::json!({
            "file_path": "new_file.txt",
            "content": "line 1\nline 2\nline 3"
        });
        let (diff_lines, is_file_tool, file_path, replace_all) =
            extract_diff_info("FileWrite", &input);
        assert!(is_file_tool);
        assert!(diff_lines.is_some());
        assert_eq!(file_path.as_deref(), Some("new_file.txt"));
        assert!(!replace_all);
        // All lines should be additions for a new file
        let diff = diff_lines.unwrap();
        assert!(diff.iter().all(|d| d.kind == crate::diff::DiffKind::Added));
    }

    #[test]
    fn test_extract_diff_info_non_file_tool() {
        let input = serde_json::json!({
            "command": "ls -la"
        });
        let (diff_lines, is_file_tool, file_path, _) = extract_diff_info("Bash", &input);
        assert!(!is_file_tool);
        assert!(diff_lines.is_none());
        assert!(file_path.is_none());
    }

    #[test]
    fn test_slash_commands_include_new_commands() {
        assert!(has_slash_command("/permissions"));
        assert!(has_slash_command("/init"));
        assert!(has_slash_command("/status"));
    }

    #[test]
    fn test_slash_command_help_includes_new_commands() {
        let help = slash_command_help_text();
        assert!(help.contains("/permissions"));
        assert!(help.contains("/init"));
        assert!(help.contains("/status"));
    }

    #[tokio::test]
    async fn test_slash_prefix_shows_suggestions() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;

        assert!(app.slash_suggestions.is_some());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_slash_prefix_filters_suggestions() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('h')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('e')), &tx).await;

        let suggestions = app.slash_suggestions.as_ref().unwrap();
        assert!(suggestions.items.iter().any(|item| item.label == "/help"));
        assert!(suggestions
            .items
            .iter()
            .all(|item| item.matches_query("he")));
    }

    #[tokio::test]
    async fn test_enter_executes_current_slash_input_without_applying_suggestion() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('h')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('e')), &tx).await;
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        assert!(rx.try_recv().is_err());
        let last = app.messages.last();
        assert!(
            matches!(last, Some(ChatMessage::System(text)) if text.contains("Unknown command: /he"))
        );
    }

    #[tokio::test]
    async fn test_down_navigates_suggestions_instead_of_history() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.history = vec!["older history".into()];
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        let initial = app.slash_suggestions.as_ref().unwrap().selected;

        app.handle_key_event(key(KeyCode::Down), &tx).await;

        assert_eq!(app.input_text(), "/");
        assert!(app.history_index.is_none());
        assert_eq!(
            app.slash_suggestions.as_ref().unwrap().selected,
            initial + 1
        );
    }

    #[tokio::test]
    async fn test_escape_clears_slash_input_even_when_suggestions_visible() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('h')), &tx).await;
        app.handle_key_event(key(KeyCode::Esc), &tx).await;

        assert!(app.input_text().is_empty());
        assert!(app.slash_suggestions.is_none());
    }

    #[tokio::test]
    async fn test_tab_applies_selected_suggestion_without_submit() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, mut rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('h')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('e')), &tx).await;
        app.handle_key_event(key(KeyCode::Tab), &tx).await;

        assert_eq!(app.input_text(), "/help");
        assert!(app.slash_suggestions.is_none());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_shift_tab_moves_suggestion_selection_backward() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        app.handle_key_event(key(KeyCode::Down), &tx).await;
        let after_down = app.slash_suggestions.as_ref().unwrap().selected;

        app.handle_key_event(key_shift(KeyCode::BackTab), &tx).await;

        assert_eq!(after_down, 1);
        assert_eq!(app.slash_suggestions.as_ref().unwrap().selected, 0);
    }

    #[tokio::test]
    async fn test_selection_auto_scrolls_when_moving_beyond_visible_window() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);
        app.terminal_height = 10;

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        for _ in 0..8 {
            app.handle_key_event(key(KeyCode::Down), &tx).await;
        }

        let suggestions = app.slash_suggestions.as_ref().unwrap();
        assert!(suggestions.selected >= suggestions.scroll);
        assert!(suggestions.scroll > 0);
    }

    #[tokio::test]
    async fn test_selection_scrolls_immediately_after_first_hidden_row() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);
        app.terminal_height = 10;

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        for _ in 0..4 {
            app.handle_key_event(key(KeyCode::Down), &tx).await;
        }

        let suggestions = app.slash_suggestions.as_ref().unwrap();
        assert_eq!(suggestions.selected, 4);
        assert!(suggestions.scroll > 0);
    }

    #[test]
    fn test_overlay_area_matches_available_terminal_space() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.terminal_width = 80;
        app.terminal_height = 10;
        app.input_buffer = InputBuffer::from_text("/");
        app.refresh_slash_suggestions();

        let area = app.slash_suggestion_overlay_area().unwrap();
        assert_eq!(area.height, 5);
    }

    #[test]
    fn test_render_row_count_includes_group_headers_and_blank_separator() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.input_buffer = InputBuffer::from_text("/");
        app.refresh_slash_suggestions();

        assert_eq!(app.slash_suggestion_render_row_count(), 29);
    }

    #[tokio::test]
    async fn test_page_down_scrolls_suggestions_without_changing_selection() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);
        app.terminal_height = 10;

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        let selected = app.slash_suggestions.as_ref().unwrap().selected;

        app.handle_key_event(key(KeyCode::PageDown), &tx).await;

        let suggestions = app.slash_suggestions.as_ref().unwrap();
        assert_eq!(suggestions.selected, selected);
        assert!(suggestions.scroll > 0);
    }

    #[test]
    fn test_mouse_scroll_inside_suggestions_scrolls_overlay() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.terminal_width = 80;
        app.terminal_height = 10;
        app.input_buffer = InputBuffer::from_text("/");
        app.refresh_slash_suggestions();

        let overlay_column = 2;
        let overlay_row = 4;
        app.handle_mouse_event(mouse_at(
            MouseEventKind::ScrollDown,
            overlay_column,
            overlay_row,
        ));

        assert!(app.slash_suggestions.as_ref().unwrap().scroll > 0);
    }

    #[test]
    fn test_mouse_scroll_inside_suggestions_does_not_scroll_chat() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        app.terminal_width = 80;
        app.terminal_height = 10;
        app.messages = (0..50)
            .map(|i| ChatMessage::Assistant(format!("msg {i}")))
            .collect();
        app.follow_output = false;
        app.scroll_offset = 5;
        app.input_buffer = InputBuffer::from_text("/");
        app.refresh_slash_suggestions();

        let overlay_column = 2;
        let overlay_row = 4;
        app.handle_mouse_event(mouse_at(
            MouseEventKind::ScrollDown,
            overlay_column,
            overlay_row,
        ));

        assert_eq!(app.scroll_offset, 5);
    }

    #[tokio::test]
    async fn test_backspace_and_paste_refresh_suggestions() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);

        app.handle_key_event(key(KeyCode::Char('/')), &tx).await;
        app.handle_key_event(key(KeyCode::Char('x')), &tx).await;
        let filtered_count = app.slash_suggestions.as_ref().unwrap().items.len();

        app.handle_key_event(key(KeyCode::Backspace), &tx).await;
        let reset_count = app.slash_suggestions.as_ref().unwrap().items.len();
        app.input_buffer.clear();
        app.slash_suggestions = None;

        app.handle_paste("/he".into());

        assert!(reset_count >= filtered_count);
        assert!(app.slash_suggestions.is_some());
        assert!(app
            .slash_suggestions
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.label == "/help"));
    }

    #[tokio::test]
    async fn test_enter_executes_exact_slash_command_without_extra_autocomplete_step() {
        let mut app = App::new(
            "test-model".into(),
            "test-model".into(),
            "default".into(),
            None,
            Theme::Dark,
        );
        let (tx, _rx) = mpsc::channel(1);

        app.input_buffer = InputBuffer::from_text("/help");
        app.refresh_slash_suggestions();
        app.handle_key_event(key(KeyCode::Enter), &tx).await;

        let last = app.messages.last();
        assert!(
            matches!(last, Some(ChatMessage::System(text)) if text.contains("Available commands:"))
        );
    }

    #[test]
    fn test_permission_dialog_diff_scroll_bounds() {
        let input = serde_json::json!({
            "file_path": "test.rs",
            "old_string": "a\nb\nc",
            "new_string": "x\ny\nz"
        });
        let (diff_lines, is_file_tool, file_path, replace_all) =
            extract_diff_info("FileEdit", &input);
        let mut dialog = PermissionDialog {
            tool_name: "FileEdit".into(),
            input_summary: "test.rs (edit)".into(),
            selected: 0,
            response_tx: None,
            diff_lines,
            diff_scroll: 0,
            is_file_tool,
            file_path,
            replace_all,
        };
        let max = dialog.diff_lines.as_ref().map(|d| d.len()).unwrap_or(0);
        // Scroll down beyond max should clamp
        dialog.diff_scroll = max + 10;
        dialog.diff_scroll = dialog.diff_scroll.min(max.saturating_sub(1));
        assert!(dialog.diff_scroll < max);
    }
}
