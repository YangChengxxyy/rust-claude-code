//! Incremental markdown parser for streaming content.
//!
//! Instead of re-parsing all text on every frame, this module maintains a
//! line-oriented state machine that processes complete lines as they arrive
//! and caches the styled `Line` output. Only the pending (incomplete) line
//! is re-parsed each frame.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::parsing::ParseState as SyntectParseState;

use crate::theme::{self, Palette};

// ── Block-level parser state ────────────────────────────────────────────────

/// Tracks which block-level context we are currently inside.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockState {
    /// Normal paragraph / top-level text.
    Paragraph,
    /// Inside a fenced code block.
    CodeBlock { lang: Option<String> },
    /// Inside a list context (informational; each line is still classified independently).
    List,
}

/// Streaming-friendly incremental markdown parser.
///
/// Call [`push_delta`] for each arriving text chunk. Previously parsed lines
/// are cached in `lines_cache` and never re-parsed. The `pending_line` holds
/// the current incomplete line (no trailing `\n` yet) and is rendered with
/// best-effort inline formatting on each frame.
#[derive(Debug, Clone)]
pub struct StreamingMarkdownState {
    /// Already-parsed lines ready for rendering.
    pub lines_cache: Vec<Line<'static>>,
    /// The current incomplete line (no newline received yet).
    pub pending_line: String,
    palette: Palette,
    /// Block-level state machine.
    block_state: BlockState,
    /// Whether the very first block has been emitted (controls blank-line
    /// insertion before headings/code-blocks).
    first_block: bool,
    /// Whether the assistant message prefix has already been emitted.
    emitted_message_prefix: bool,
    /// Tracks whether the most recent cached item was a blank line,
    /// to avoid duplicate blank lines.
    last_was_blank: bool,
    /// Syntect parse state for incremental code block highlighting.
    highlight_state: Option<SyntectParseState>,
    /// Syntect highlight state for correct multi-line construct coloring.
    highlight_hl_state: Option<syntect::highlighting::HighlightState>,
    /// Cached theme for the current code block (avoids rebuilding per line).
    highlight_theme: Option<syntect::highlighting::Theme>,
    /// The syntax reference for the current code block (cached for the block duration).
    highlight_syntax_name: Option<String>,
}

impl Default for StreamingMarkdownState {
    fn default() -> Self {
        Self::new(Palette::dark())
    }
}

impl StreamingMarkdownState {
    pub fn new(palette: Palette) -> Self {
        Self {
            lines_cache: Vec::new(),
            pending_line: String::new(),
            palette,
            block_state: BlockState::Paragraph,
            first_block: true,
            emitted_message_prefix: false,
            last_was_blank: false,
            highlight_state: None,
            highlight_hl_state: None,
            highlight_theme: None,
            highlight_syntax_name: None,
        }
    }

    /// Returns `true` if there is any content (cached lines or pending text).
    pub fn is_empty(&self) -> bool {
        self.lines_cache.is_empty() && self.pending_line.is_empty()
    }

    /// Returns the total number of cached lines (excluding pending).
    pub fn cached_line_count(&self) -> usize {
        self.lines_cache.len()
    }

    /// Collect the final text from accumulated content.
    /// Used when converting streaming state to a ChatMessage.
    pub fn take_text(&mut self) -> String {
        // We reconstruct the raw text from the pending line and a rough
        // approximation. For correctness, we keep a raw buffer too.
        // But since the actual ChatMessage stores the raw text and the
        // markdown rendering is done again by render_markdown_message,
        // we don't need perfect fidelity here.
        let mut result = String::new();
        // The raw text was already consumed line-by-line, so we cannot
        // perfectly reconstruct it. Instead, callers should maintain a
        // separate raw text accumulator alongside this state.
        result.push_str(&self.pending_line);
        result
    }

    /// Push a new streaming delta into the parser.
    ///
    /// The delta is split by newlines. Complete lines are parsed through the
    /// block state machine and their styled `Line` output is appended to
    /// `lines_cache`. The trailing fragment (if no trailing newline) is
    /// stored in `pending_line`.
    pub fn push_delta(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // Prepend any pending content to the incoming text
        let full_text = if self.pending_line.is_empty() {
            text.to_string()
        } else {
            let mut s = std::mem::take(&mut self.pending_line);
            s.push_str(text);
            s
        };

        // Split into lines. The last element may be an incomplete line.
        let mut parts: Vec<&str> = full_text.split('\n').collect();

        // If the text ends with '\n', the last element is "" which means
        // all lines are complete.
        let last = parts.pop().unwrap_or("");
        self.pending_line = last.to_string();

        // Process all complete lines
        for line in parts {
            self.process_complete_line(line);
        }
    }

    /// Process a single complete line through the block state machine.
    fn process_complete_line(&mut self, line: &str) {
        match &self.block_state {
            BlockState::CodeBlock { .. } => {
                // Check for closing fence
                if line.trim_start().starts_with("```") {
                    // Close the code block
                    self.lines_cache.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("└─", Style::default().fg(self.palette.bash_border)),
                    ]));
                    self.block_state = BlockState::Paragraph;
                    self.last_was_blank = false;
                    self.highlight_state = None;
                    self.highlight_hl_state = None;
                    self.highlight_theme = None;
                    self.highlight_syntax_name = None;
                } else {
                    // Code line inside block — use incremental highlighting
                    let did_highlight = if let (Some(parse_state), Some(hl_state), Some(theme)) = (
                        self.highlight_state.as_mut(),
                        self.highlight_hl_state.as_mut(),
                        self.highlight_theme.as_ref(),
                    ) {
                        let highlighter = syntect::highlighting::Highlighter::new(theme);
                        let spans_data = crate::highlight::highlight_line(
                            &format!("{}\n", line),
                            parse_state,
                            hl_state,
                            &highlighter,
                        );
                        let mut spans = vec![
                            Span::raw("  "),
                            Span::styled("│ ", Style::default().fg(self.palette.bash_border)),
                        ];
                        for (style, text) in spans_data {
                            let text = text.trim_end_matches('\n').to_string();
                            if !text.is_empty() {
                                spans.push(Span::styled(text, style));
                            }
                        }
                        self.lines_cache.push(Line::from(spans));
                        self.last_was_blank = false;
                        true
                    } else {
                        false
                    };
                    if !did_highlight {
                        // Fallback to monochrome (no highlight state available)
                        self.lines_cache.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled("│ ", Style::default().fg(self.palette.bash_border)),
                            Span::styled(line.to_string(), Style::default().fg(self.palette.text)),
                        ]));
                        self.last_was_blank = false;
                    }
                }
            }
            BlockState::Paragraph | BlockState::List => {
                let trimmed = line.trim();

                // Empty line
                if trimmed.is_empty() {
                    if !self.last_was_blank {
                        self.lines_cache.push(Line::from(""));
                        self.last_was_blank = true;
                    }
                    self.block_state = BlockState::Paragraph;
                    return;
                }

                // Code fence opening
                if let Some(rest) = trimmed.strip_prefix("```") {
                    if !self.first_block && !self.last_was_blank {
                        self.lines_cache.push(Line::from(""));
                    }
                    let lang = if rest.trim().is_empty() {
                        None
                    } else {
                        Some(rest.trim().to_string())
                    };
                    let title = lang.as_deref().unwrap_or("code");
                    self.lines_cache.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("┌─ {title}"),
                            Style::default().fg(self.palette.bash_border),
                        ),
                    ]));
                    // Initialize syntect parse + highlight state for incremental highlighting
                    if let Some(ref l) = lang {
                        let ss = crate::highlight::syntax_set();
                        if let Some(syntax) = crate::highlight::resolve_syntax(l, ss) {
                            self.highlight_state = Some(SyntectParseState::new(syntax));
                            self.highlight_syntax_name = Some(syntax.name.clone());
                            // Cache theme and create initial highlight state
                            let theme = crate::highlight::build_custom_theme(&self.palette);
                            let highlighter = syntect::highlighting::Highlighter::new(&theme);
                            self.highlight_hl_state = Some(crate::highlight::new_highlight_state(&highlighter));
                            self.highlight_theme = Some(theme);
                        }
                    }
                    self.block_state = BlockState::CodeBlock { lang };
                    self.first_block = false;
                    self.last_was_blank = false;
                    return;
                }

                // Heading
                let heading_level = trimmed.chars().take_while(|c| *c == '#').count();
                if (1..=3).contains(&heading_level)
                    && trimmed.chars().nth(heading_level) == Some(' ')
                {
                    if !self.first_block && !self.last_was_blank {
                        self.lines_cache.push(Line::from(""));
                    }
                    let heading_text = trimmed[heading_level + 1..].to_string();
                    let style = match heading_level {
                        1 => Style::default()
                            .fg(self.palette.claude)
                            .add_modifier(Modifier::BOLD),
                        2 => Style::default()
                            .fg(self.palette.suggestion)
                            .add_modifier(Modifier::BOLD),
                        _ => Style::default()
                            .fg(self.palette.text)
                            .add_modifier(Modifier::BOLD),
                    };
                    let prefix = if !self.emitted_message_prefix {
                        self.emitted_message_prefix = true;
                        Span::styled(
                            format!("{} ", theme::ASSISTANT_BULLET),
                            self.palette.bullet_style(),
                        )
                    } else {
                        Span::raw("  ")
                    };
                    self.lines_cache.push(Line::from(vec![
                        prefix,
                        Span::styled(heading_text, style),
                    ]));
                    self.block_state = BlockState::Paragraph;
                    self.first_block = false;
                    self.last_was_blank = false;
                    return;
                }

                // Unordered list item
                if let Some(text) =
                    trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* "))
                {
                    let mut spans = vec![
                        Span::raw("  "),
                        Span::styled(
                            "• ".to_string(),
                            Style::default().fg(self.palette.suggestion),
                        ),
                    ];
                    spans.extend(parse_inline_spans(text, self.palette));
                    self.lines_cache.push(Line::from(spans));
                    self.block_state = BlockState::List;
                    self.first_block = false;
                    self.last_was_blank = false;
                    return;
                }

                // Ordered list item
                if let Some((marker, rest)) = parse_ordered_list_item(trimmed) {
                    let mut spans = vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("{marker} "),
                            Style::default().fg(self.palette.suggestion),
                        ),
                    ];
                    spans.extend(parse_inline_spans(&rest, self.palette));
                    self.lines_cache.push(Line::from(spans));
                    self.block_state = BlockState::List;
                    self.first_block = false;
                    self.last_was_blank = false;
                    return;
                }

                // Regular paragraph line
                let mut spans = Vec::new();
                if self.first_block && self.lines_cache.is_empty() {
                    // Very first line of the entire stream — bullet prefix
                    spans.push(Span::styled(
                        format!("{} ", theme::ASSISTANT_BULLET),
                        self.palette.bullet_style(),
                    ));
                    self.emitted_message_prefix = true;
                } else if self.last_was_blank || self.first_block {
                    // First line of a new paragraph — indent only
                    spans.push(Span::raw("  "));
                } else {
                    // Continuation line — indent
                    spans.push(Span::raw("  "));
                }
                spans.extend(parse_inline_spans(line, self.palette));
                self.lines_cache.push(Line::from(spans));
                self.first_block = false;
                self.last_was_blank = false;
            }
        }
    }

    /// Render the pending (incomplete) line with best-effort inline formatting.
    ///
    /// Returns `None` if the pending line is empty.
    pub fn render_pending_line(&self) -> Option<Line<'static>> {
        if self.pending_line.is_empty() {
            return None;
        }

        match &self.block_state {
            BlockState::CodeBlock { .. } => {
                // Inside a code block — render as code line
                Some(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("│ ", Style::default().fg(self.palette.bash_border)),
                    Span::styled(
                        self.pending_line.clone(),
                        Style::default().fg(self.palette.text),
                    ),
                ]))
            }
            _ => {
                // Paragraph / list context — apply inline formatting
                let trimmed = self.pending_line.trim();

                // If it looks like a heading start, render with heading style
                let heading_level = trimmed.chars().take_while(|c| *c == '#').count();
                if (1..=3).contains(&heading_level)
                    && trimmed.chars().nth(heading_level) == Some(' ')
                {
                    let heading_text = &trimmed[heading_level + 1..];
                    let style = match heading_level {
                        1 => Style::default()
                            .fg(self.palette.claude)
                            .add_modifier(Modifier::BOLD),
                        2 => Style::default()
                            .fg(self.palette.suggestion)
                            .add_modifier(Modifier::BOLD),
                        _ => Style::default()
                            .fg(self.palette.text)
                            .add_modifier(Modifier::BOLD),
                    };
                    return Some(Line::from(vec![
                        if !self.emitted_message_prefix {
                            Span::styled(
                                format!("{} ", theme::ASSISTANT_BULLET),
                                self.palette.bullet_style(),
                            )
                        } else {
                            Span::raw("  ")
                        },
                        Span::styled(heading_text.to_string(), style),
                    ]));
                }

                // Regular line with inline formatting
                let mut spans = Vec::new();
                if self.lines_cache.is_empty() && self.first_block {
                    spans.push(Span::styled(
                        format!("{} ", theme::ASSISTANT_BULLET),
                        self.palette.bullet_style(),
                    ));
                } else if self.last_was_blank {
                    spans.push(Span::raw("  "));
                } else {
                    spans.push(Span::raw("  "));
                }
                spans.extend(parse_inline_spans(&self.pending_line, self.palette));
                Some(Line::from(spans))
            }
        }
    }

    /// Clear all state, preparing for reuse.
    pub fn clear(&mut self) {
        self.lines_cache.clear();
        self.pending_line.clear();
        self.block_state = BlockState::Paragraph;
        self.first_block = true;
        self.emitted_message_prefix = false;
        self.last_was_blank = false;
        self.highlight_state = None;
        self.highlight_hl_state = None;
        self.highlight_theme = None;
        self.highlight_syntax_name = None;
    }
}

// ── Inline span parser ──────────────────────────────────────────────────────

/// Parse inline markdown spans (code, bold, italic) within a single line.
///
/// This is a standalone version of the parser from `ui.rs`, made public
/// for use by the streaming markdown module.
pub fn parse_inline_spans(text: &str, palette: Palette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut plain = String::new();

    while i < chars.len() {
        // Inline code: `code`
        if chars[i] == '`' {
            if !plain.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut plain),
                    palette.assistant_text_style(),
                ));
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '`' {
                j += 1;
            }
            if j < chars.len() {
                let content: String = chars[i + 1..j].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().fg(palette.text).bg(palette.user_msg_bg),
                ));
                i = j + 1;
                continue;
            }
        }

        // Bold: **text**
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !plain.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut plain),
                    palette.assistant_text_style(),
                ));
            }
            let mut j = i + 2;
            while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '*') {
                j += 1;
            }
            if j + 1 < chars.len() {
                let content: String = chars[i + 2..j].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default()
                        .fg(palette.text)
                        .add_modifier(Modifier::BOLD),
                ));
                i = j + 2;
                continue;
            }
        }

        // Italic: *text*
        if chars[i] == '*' {
            if !plain.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut plain),
                    palette.assistant_text_style(),
                ));
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '*' {
                j += 1;
            }
            if j < chars.len() {
                let content: String = chars[i + 1..j].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default()
                        .fg(palette.text)
                        .add_modifier(Modifier::ITALIC),
                ));
                i = j + 1;
                continue;
            }
        }

        plain.push(chars[i]);
        i += 1;
    }

    if !plain.is_empty() {
        spans.push(Span::styled(plain, palette.assistant_text_style()));
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }
    spans
}

/// Parse an ordered list item like "1. text" or "23. text".
fn parse_ordered_list_item(line: &str) -> Option<(String, String)> {
    let mut digits = String::new();
    for ch in line.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let suffix = &line[digits.len()..];
    let rest = suffix.strip_prefix(". ")?;
    Some((format!("{digits}."), rest.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_delta_basic_text() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("Hello world\n");
        assert_eq!(state.lines_cache.len(), 1);
        assert!(state.pending_line.is_empty());
    }

    #[test]
    fn test_push_delta_partial_line() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("Hello");
        assert!(state.lines_cache.is_empty());
        assert_eq!(state.pending_line, "Hello");

        state.push_delta(" world\n");
        assert_eq!(state.lines_cache.len(), 1);
        assert!(state.pending_line.is_empty());
    }

    #[test]
    fn test_heading_detection() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("## My Heading\n");
        assert_eq!(state.lines_cache.len(), 1);
        // The line should contain styled heading spans
        let line = &state.lines_cache[0];
        assert!(line.spans.len() >= 2);
    }

    #[test]
    fn test_code_block_state_across_deltas() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("```rust\n");
        assert_eq!(state.block_state, BlockState::CodeBlock { lang: Some("rust".to_string()) });

        state.push_delta("fn main() {\n");
        state.push_delta("    println!(\"hello\");\n");
        state.push_delta("}\n");
        // Opening fence line + 3 code lines = 4 lines (fence becomes the ┌─ line)
        assert_eq!(state.lines_cache.len(), 4);

        state.push_delta("```\n");
        // +1 for closing └─ line
        assert_eq!(state.lines_cache.len(), 5);
        assert_eq!(state.block_state, BlockState::Paragraph);
    }

    #[test]
    fn test_unordered_list() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("- item one\n- item two\n");
        assert_eq!(state.lines_cache.len(), 2);
    }

    #[test]
    fn test_ordered_list() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("1. first\n2. second\n");
        assert_eq!(state.lines_cache.len(), 2);
    }

    #[test]
    fn test_inline_bold() {
        let spans = parse_inline_spans("hello **world** there", Palette::dark());
        assert!(spans.len() >= 3);
    }

    #[test]
    fn test_inline_code() {
        let spans = parse_inline_spans("use `foo` here", Palette::dark());
        assert!(spans.len() >= 3);
    }

    #[test]
    fn test_inline_italic() {
        let spans = parse_inline_spans("this is *italic* text", Palette::dark());
        assert!(spans.len() >= 3);
    }

    #[test]
    fn test_render_pending_line_empty() {
        let state = StreamingMarkdownState::new(Palette::dark());
        assert!(state.render_pending_line().is_none());
    }

    #[test]
    fn test_render_pending_line_with_content() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("Hello partial");
        let line = state.render_pending_line();
        assert!(line.is_some());
    }

    #[test]
    fn test_render_pending_line_in_code_block() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("```rust\nfn main");
        let line = state.render_pending_line();
        assert!(line.is_some());
        // Should have code block styling (│ prefix)
        let line = line.unwrap();
        assert!(line.spans.len() >= 2);
    }

    #[test]
    fn test_is_empty() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        assert!(state.is_empty());
        state.push_delta("x");
        assert!(!state.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("## Heading\nSome text\n");
        assert!(!state.is_empty());
        state.clear();
        assert!(state.is_empty());
        assert_eq!(state.lines_cache.len(), 0);
    }

    #[test]
    fn test_multiple_paragraphs() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("First paragraph.\n\nSecond paragraph.\n");
        // line 1: "First paragraph." (with bullet)
        // line 2: blank
        // line 3: "Second paragraph." (with bullet for new paragraph)
        assert_eq!(state.lines_cache.len(), 3);
    }

    #[test]
    fn test_second_paragraph_uses_indent_not_second_prefix() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("First paragraph.\n\nSecond paragraph.\n");

        assert_eq!(state.lines_cache[0].spans[0].content.as_ref(), "• ");
        assert_eq!(state.lines_cache[2].spans[0].content.as_ref(), "  ");
    }

    #[test]
    fn test_line_cache_growth() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        for i in 0..500 {
            state.push_delta(&format!("Line {i}\n"));
        }
        assert_eq!(state.lines_cache.len(), 500);
    }

    #[test]
    fn test_heading_pending_line() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        state.push_delta("## Heading in progress");
        let pending = state.render_pending_line().unwrap();
        // Should be styled as heading
        assert!(pending.spans.len() >= 2);
    }

    #[test]
    fn test_performance_500_lines() {
        let mut state = StreamingMarkdownState::new(Palette::dark());
        // Push 500 lines of mixed markdown content
        for i in 0..100 {
            state.push_delta(&format!("## Heading {i}\n"));
            state.push_delta(&format!("Paragraph line with **bold** and `code` for item {i}.\n"));
            state.push_delta(&format!("- List item {i}\n"));
            state.push_delta(&format!("```rust\nfn example_{i}() {{}}\n```\n"));
            state.push_delta("\n");
        }
        assert_eq!(state.lines_cache.len(), 500 + 100 + 100 + 100); // heading+para+list+fence_open+code+fence_close+blank per iteration

        // Measure render_pending_line + cache access
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = state.render_pending_line();
            let _count = state.lines_cache.len();
        }
        let elapsed = start.elapsed();
        // Must be well under 50ms for 100 iterations
        assert!(
            elapsed.as_millis() < 50,
            "Performance test failed: 100 render_pending_line calls took {}ms (limit: 50ms)",
            elapsed.as_millis()
        );
    }
}
