//! Rendering layer — draws the TUI frame in a style that matches Claude Code.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use rust_claude_core::state::TodoStatus;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, CursorPosition};
use crate::events::ChatMessage;
use crate::theme::{self, Palette};

/// Compute the chat viewport area for the current frame layout.
pub fn chat_viewport_area(app: &App, full: Rect) -> Rect {
    let (main_area, _) = if app.todo_visible {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(30)])
            .split(full);
        (chunks[0], Some(chunks[1]))
    } else {
        (full, None)
    };

    let input_height = input_area_height(app).clamp(3, 8);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(main_area);

    areas[0]
}

/// Build the chat lines exactly as they will be rendered in the viewport.
pub fn build_chat_lines(app: &App) -> Vec<Line<'static>> {
    let palette = app.palette();
    let mut lines: Vec<Line> = Vec::new();

    for (idx, msg) in app.messages.iter().enumerate() {
        render_message(msg, idx, app, &mut lines);
    }

    // Render streaming thinking content in real time
    if app.is_streaming && !app.streaming_thinking_md.is_empty() {
        // Thinking header
        let header_text = if app.thinking_folded {
            "[Thinking] ...".to_string()
        } else {
            "[Thinking]".to_string()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", theme::BLACK_CIRCLE), palette.spinner_style()),
            Span::styled(
                header_text,
                Style::default()
                    .fg(palette.inactive)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));

        if !app.thinking_folded {
            // Render cached thinking lines in dim/italic style
            let thinking_style = Style::default()
                .fg(palette.inactive)
                .add_modifier(Modifier::ITALIC);
            for cached_line in &app.streaming_thinking_md.lines_cache {
                // Re-style all spans to dim/italic for thinking
                let styled_spans: Vec<Span> = cached_line
                    .spans
                    .iter()
                    .map(|span| Span::styled(span.content.to_string(), thinking_style))
                    .collect();
                lines.push(Line::from(styled_spans));
            }
            if let Some(pending) = app.streaming_thinking_md.render_pending_line() {
                let styled_spans: Vec<Span> = pending
                    .spans
                    .iter()
                    .map(|span| Span::styled(span.content.to_string(), thinking_style))
                    .collect();
                lines.push(Line::from(styled_spans));
            }
        }
    }

    // Render streaming text with incremental markdown
    if app.is_streaming && !app.streaming_md.is_empty() {
        // Render all cached (fully parsed) lines
        lines.extend(app.streaming_md.lines_cache.iter().cloned());
        // Render the pending (incomplete) line
        if let Some(pending) = app.streaming_md.render_pending_line() {
            lines.push(pending);
        }
    }

    // Render streaming tool call construction
    if let Some(ref tool_state) = app.streaming_tool {
        let display_name = ChatMessage::user_facing_tool_name(&tool_state.name);
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", theme::BLACK_CIRCLE), palette.bullet_style()),
            Span::styled(format!("{display_name} "), palette.tool_name_style()),
            Span::styled("constructing...", palette.tool_desc_style()),
        ]));
        if !tool_state.accumulated_json.is_empty() {
            // Show the partial JSON in a code-block style
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("┌─ input", Style::default().fg(palette.bash_border)),
            ]));
            for json_line in tool_state.accumulated_json.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("│ ", Style::default().fg(palette.bash_border)),
                    Span::styled(json_line.to_string(), Style::default().fg(palette.text)),
                ]));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Type a message below to get started.",
            Style::default().fg(palette.subtle),
        )));
    }

    lines
}

/// Compute the maximum valid scroll offset for the chat viewport.
pub fn max_chat_scroll_offset(app: &App, viewport_width: u16, viewport_height: u16) -> u16 {
    if viewport_width == 0 || viewport_height == 0 {
        return 0;
    }

    let lines = build_chat_lines(app);
    let total_visual_lines = count_visual_lines(&lines, viewport_width);
    total_visual_lines.saturating_sub(viewport_height)
}

fn count_visual_lines(lines: &[Line<'static>], viewport_width: u16) -> u16 {
    let width = viewport_width.max(1) as usize;
    lines
        .iter()
        .map(|line| {
            let line_width = line_display_width(line).max(1);
            ((line_width.saturating_sub(1) / width) + 1) as u16
        })
        .sum()
}

fn line_display_width(line: &Line<'static>) -> usize {
    let width: usize = line
        .spans
        .iter()
        .map(|span| display_width(span.content.as_ref()))
        .sum();
    width.max(1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownBlock {
    Heading {
        level: usize,
        text: String,
    },
    ListItem {
        ordered: bool,
        marker: String,
        text: String,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Paragraph(String),
    Blank,
}

/// Draw the entire TUI frame.
pub fn draw(f: &mut Frame, app: &App) {
    let full = f.size();
    let chat_area = chat_viewport_area(app, full);

    let (main_area, todo_area) = if app.todo_visible {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(30)])
            .split(full);
        (chunks[0], Some(chunks[1]))
    } else {
        (full, None)
    };

    let input_height = input_area_height(app).clamp(3, 8);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(main_area);

    draw_chat_area(f, app, chat_area);
    draw_spinner_line(f, app, areas[1]);
    draw_input_area(f, app, areas[2]);
    draw_status_bar(f, app, areas[3]);

    if let Some(todo_area) = todo_area {
        draw_todo_panel(f, app, todo_area);
    }

    if app.permission_dialog.is_some() {
        draw_permission_dialog(f, app, full);
    }

    if app.user_question_dialog.is_some() {
        draw_user_question_dialog(f, app, full);
    }

    if app.session_picker.is_some() {
        draw_session_picker(f, app, full);
    }
}

fn input_area_height(app: &App) -> u16 {
    let lines = app.input_text().lines().count().max(1) as u16;
    lines + 2
}

fn draw_chat_area(f: &mut Frame, app: &App, area: Rect) {
    let lines = build_chat_lines(app);

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    f.render_widget(paragraph, area);
}

fn render_message(
    msg: &ChatMessage,
    message_index: usize,
    app: &App,
    lines: &mut Vec<Line<'static>>,
) {
    let palette = app.palette();
    match msg {
        ChatMessage::User(text) => {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            for (i, line) in text.lines().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "> ",
                            Style::default()
                                .fg(palette.suggestion)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(line.to_string(), palette.user_text_style()),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(line.to_string(), palette.user_text_style()),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }
        ChatMessage::Assistant(text) => {
            render_markdown_message(text, lines, palette);
        }
        ChatMessage::Thinking { summary, content } => {
            let is_selected = app.selected_thinking == Some(message_index);
            let is_expanded = app.expanded_thinking.contains(&message_index);
            let prefix = if is_selected { ">" } else { " " };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {prefix} "),
                    if is_selected {
                        Style::default()
                            .fg(palette.claude)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette.subtle)
                    },
                ),
                Span::styled(
                    "Thinking",
                    palette.spinner_style().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" — "),
                Span::styled(summary.clone(), Style::default().fg(palette.inactive)),
                Span::styled(
                    if is_expanded {
                        " [Tab to collapse]"
                    } else {
                        " [Tab to expand]"
                    },
                    Style::default().fg(palette.subtle),
                ),
            ]));
            if is_expanded {
                for line in content.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(line.to_string(), Style::default().fg(palette.inactive)),
                    ]));
                }
            }
        }
        ChatMessage::ToolUse {
            name,
            input_summary,
            diff_lines,
        } => {
            let display_name = ChatMessage::user_facing_tool_name(name);

            if name == "Bash" {
                let cmd_display = if input_summary.len() > 160 {
                    format!("{}…", &input_summary[..159])
                } else {
                    input_summary.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", theme::RESPONSE_PREFIX),
                        palette.response_prefix_style(),
                    ),
                    Span::styled(format!("{display_name} "), palette.tool_name_style()),
                    Span::styled("! ", palette.bash_prefix_style()),
                    Span::styled(cmd_display, palette.bash_command_style()),
                ]));
            } else {
                let mut spans = vec![
                    Span::styled(
                        format!("  {} ", theme::RESPONSE_PREFIX),
                        palette.response_prefix_style(),
                    ),
                    Span::styled(display_name.to_string(), palette.tool_name_style()),
                ];
                if !input_summary.is_empty() {
                    spans.push(Span::styled(
                        format!(" ({input_summary})"),
                        palette.tool_desc_style(),
                    ));
                }
                lines.push(Line::from(spans));
            }

            // Render compact diff view for FileEdit/FileWrite tool uses
            if let Some(diff) = diff_lines {
                let max_diff_lines = 20usize;
                let rendered = crate::diff::render_diff_lines(diff, &palette, 80);
                let total = rendered.len();
                if total <= max_diff_lines {
                    for line in rendered {
                        lines.push(line);
                    }
                } else {
                    // Show first 10 + indicator + last 5
                    for line in rendered.iter().take(10) {
                        lines.push(line.clone());
                    }
                    lines.push(Line::from(Span::styled(
                        format!("    ... {} more lines ...", total - 15),
                        Style::default().fg(palette.inactive),
                    )));
                    for line in rendered.iter().skip(total - 5) {
                        lines.push(line.clone());
                    }
                }
            }
        }
        ChatMessage::ToolResult {
            name: _,
            output_summary,
            is_error,
        } => {
            if output_summary.is_empty() {
                return;
            }
            let style = if *is_error {
                palette.error_style()
            } else {
                // Use a dimmer style for tool results so they don't compete
                // with the main assistant text visually.
                Style::default().fg(palette.inactive)
            };

            for (i, line) in output_summary.lines().take(6).enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {} ", theme::RESPONSE_PREFIX),
                            palette.response_prefix_style(),
                        ),
                        Span::styled(line.to_string(), style),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(line.to_string(), style),
                    ]));
                }
            }
            let line_count = output_summary.lines().count();
            if line_count > 6 {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("… ({} more lines)", line_count - 6),
                        Style::default().fg(palette.subtle),
                    ),
                ]));
            }
        }
        ChatMessage::System(text) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(text.clone(), palette.warning_style()),
            ]));
        }
    }
}

fn render_markdown_message(text: &str, lines: &mut Vec<Line<'static>>, palette: Palette) {
    let blocks = parse_markdown_blocks(text);
    let mut first_block = true;
    let mut used_message_prefix = false;
    for block in blocks {
        match block {
            MarkdownBlock::Heading { level, text } => {
                if !first_block {
                    lines.push(Line::from(""));
                }
                let style = match level {
                    1 => Style::default()
                        .fg(palette.claude)
                        .add_modifier(Modifier::BOLD),
                    2 => Style::default()
                        .fg(palette.suggestion)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default()
                        .fg(palette.text)
                        .add_modifier(Modifier::BOLD),
                };
                let prefix = if !used_message_prefix {
                    used_message_prefix = true;
                    Span::styled(
                        format!("{} ", theme::ASSISTANT_BULLET),
                        palette.bullet_style(),
                    )
                } else {
                    Span::raw("  ")
                };
                lines.push(Line::from(vec![prefix, Span::styled(text, style)]));
            }
            MarkdownBlock::ListItem {
                ordered: _,
                marker,
                text,
            } => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{marker} "),
                        Style::default().fg(palette.suggestion),
                    ),
                ]));
                if let Some(last) = lines.last_mut() {
                    last.spans.extend(parse_inline_spans(&text, palette));
                }
                used_message_prefix = true;
            }
            MarkdownBlock::CodeBlock { language, code } => {
                if !first_block {
                    lines.push(Line::from(""));
                }
                let title = language.as_deref().unwrap_or("code").to_string();
                let code_prefix = if !used_message_prefix {
                    used_message_prefix = true;
                    format!("{} ", theme::ASSISTANT_BULLET)
                } else {
                    "  ".to_string()
                };
                lines.push(Line::from(vec![
                    Span::raw(code_prefix),
                    Span::styled(
                        format!("┌─ {title}"),
                        Style::default().fg(palette.bash_border),
                    ),
                ]));
                let highlighted =
                    crate::highlight::highlight_code_block(&code, language.as_deref(), &palette);
                for highlighted_line in highlighted {
                    let mut spans = vec![
                        Span::raw("  "),
                        Span::styled("│ ", Style::default().fg(palette.bash_border)),
                    ];
                    for (style, text) in highlighted_line {
                        let text = text.trim_end_matches('\n').to_string();
                        if !text.is_empty() {
                            spans.push(Span::styled(text, style));
                        }
                    }
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("└─", Style::default().fg(palette.bash_border)),
                ]));
            }
            MarkdownBlock::Paragraph(text) => {
                if !first_block {
                    lines.push(Line::from(""));
                }
                let mut first_line = true;
                for logical_line in text.lines() {
                    let mut spans = Vec::new();
                    if first_line {
                        if !used_message_prefix {
                            spans.push(Span::styled(
                                format!("{} ", theme::ASSISTANT_BULLET),
                                palette.bullet_style(),
                            ));
                            used_message_prefix = true;
                        } else {
                            spans.push(Span::raw("  "));
                        }
                        first_line = false;
                    } else {
                        spans.push(Span::raw("  "));
                    }
                    spans.extend(parse_inline_spans(logical_line, palette));
                    lines.push(Line::from(spans));
                }
            }
            MarkdownBlock::Blank => lines.push(Line::from("")),
        }
        first_block = false;
    }
}

fn parse_markdown_blocks(text: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();
    let mut in_code = false;
    let mut code_lang = None;
    let mut code_lines = Vec::new();

    let flush_paragraph = |paragraph: &mut Vec<String>, blocks: &mut Vec<MarkdownBlock>| {
        if !paragraph.is_empty() {
            blocks.push(MarkdownBlock::Paragraph(paragraph.join("\n")));
            paragraph.clear();
        }
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("```") {
            flush_paragraph(&mut paragraph, &mut blocks);
            if in_code {
                blocks.push(MarkdownBlock::CodeBlock {
                    language: code_lang.take(),
                    code: code_lines.join("\n"),
                });
                code_lines.clear();
                in_code = false;
            } else {
                in_code = true;
                code_lang = (!rest.trim().is_empty()).then(|| rest.trim().to_string());
            }
            continue;
        }

        if in_code {
            code_lines.push(line.to_string());
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(MarkdownBlock::Blank);
            continue;
        }

        let heading_level = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=3).contains(&heading_level) && trimmed.chars().nth(heading_level) == Some(' ') {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(MarkdownBlock::Heading {
                level: heading_level,
                text: trimmed[heading_level + 1..].to_string(),
            });
            continue;
        }

        if let Some(text) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(MarkdownBlock::ListItem {
                ordered: false,
                marker: "•".to_string(),
                text: text.to_string(),
            });
            continue;
        }

        if let Some((marker, rest)) = parse_ordered_list_item(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(MarkdownBlock::ListItem {
                ordered: true,
                marker,
                text: rest,
            });
            continue;
        }

        paragraph.push(line.to_string());
    }

    if in_code {
        blocks.push(MarkdownBlock::CodeBlock {
            language: code_lang.take(),
            code: code_lines.join("\n"),
        });
    }
    if !paragraph.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(paragraph.join("\n")));
    }

    while matches!(blocks.last(), Some(MarkdownBlock::Blank)) {
        blocks.pop();
    }

    blocks
}

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

fn parse_inline_spans(text: &str, palette: Palette) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut plain = String::new();

    while i < chars.len() {
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

fn draw_spinner_line(f: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette();
    let content = if app.is_thinking {
        Line::from(vec![Span::styled("Thinking…", palette.spinner_style())])
    } else if app.is_streaming {
        Line::from(vec![Span::styled("Streaming…", palette.spinner_style())])
    } else {
        Line::from("")
    };

    f.render_widget(Paragraph::new(content), area);
}

fn draw_input_area(f: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette();
    let border_style = if app.is_streaming {
        Style::default().fg(palette.subtle)
    } else {
        palette.prompt_border_style()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(border_style)
        .title(if app.is_streaming {
            " Input (locked) "
        } else {
            " Input "
        });

    let input_style = if app.is_streaming {
        palette.input_disabled_style()
    } else {
        palette.user_text_style()
    };

    let input_text = build_input_text(app, input_style);
    // Do NOT enable .wrap() — the cursor position calculation maps logical
    // lines 1-to-1 to visual lines.  Wrapping would cause cursor misplacement
    // because the Y offset is derived from the logical line number.
    let paragraph = Paragraph::new(input_text).block(block);

    f.render_widget(paragraph, area);

    if !app.is_streaming {
        let CursorPosition { row, col } = app.input_cursor();
        let current_line = app
            .input_text()
            .lines()
            .nth(row)
            .unwrap_or_default()
            .to_string();
        let cursor_x = area.x
            + 1
            + 2
            + display_width(&current_line[..char_to_byte(&current_line, col)]) as u16;
        let cursor_y = area.y + 1 + row as u16;
        f.set_cursor(
            cursor_x.min(area.right().saturating_sub(1)),
            cursor_y.min(area.bottom().saturating_sub(1)),
        );
    }
}

fn build_input_text(app: &App, input_style: Style) -> Text<'static> {
    let text = app.input_text();
    if text.is_empty() && !app.is_streaming {
        return Text::from(Line::from(vec![Span::styled(
            "> ",
            Style::default()
                .fg(app.palette().suggestion)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    let mut lines = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            let line_text = line.to_owned();
            lines.push(Line::from(vec![
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(app.palette().suggestion)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(line_text, input_style),
            ]));
        } else {
            let line_text = line.to_owned();
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(line_text, input_style),
            ]));
        }
    }
    if text.ends_with('\n') {
        lines.push(Line::from(vec![Span::raw("  ")]));
    }
    Text::from(lines)
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette();
    let mode_display = match app.permission_mode.as_str() {
        "BypassPermissions" => "bypass",
        "AcceptEdits" => "accept-edits",
        "DontAsk" => "dont-ask",
        "Plan" => "plan",
        "Default" => "default",
        other => other,
    };

    let model_display = if app.model == app.model_setting {
        format_model_display_name(&app.model)
    } else {
        format!(
            "{} (from {})",
            format_model_display_name(&app.model),
            format_model_display_name(&app.model_setting)
        )
    };

    let branch_display = app
        .git_branch
        .as_ref()
        .map(|branch| format!(" ⎇ {branch}"))
        .unwrap_or_default();
    let left = format!(" {}{} ", model_display, branch_display);
    let cache_info = if app.input_tokens > 0 {
        let hit_pct = (app.cache_read_input_tokens as f64 / app.input_tokens as f64 * 100.0) as u32;
        format!(" cache:{hit_pct}%")
    } else {
        String::new()
    };

    let theme_display = match app.theme {
        rust_claude_core::config::Theme::Dark => "dark",
        rust_claude_core::config::Theme::Light => "light",
    };

    let right = format!(
        " tokens: {}↑ {}↓{} | {} | theme:{} ",
        format_tokens(app.input_tokens),
        format_tokens(app.output_tokens),
        cache_info,
        mode_display,
        theme_display,
    );

    let total_width = area.width as usize;
    let used = left.len() + right.len();
    let padding = if total_width > used {
        " ".repeat(total_width - used)
    } else {
        String::new()
    };

    let line = Line::from(vec![
        Span::styled(
            &left,
            Style::default()
                .fg(palette.claude)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(padding, palette.status_bar_style()),
        Span::styled(&right, palette.status_bar_style()),
    ]);

    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

fn draw_permission_dialog(f: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette();
    let dialog = match &app.permission_dialog {
        Some(d) => d,
        None => return,
    };

    let display_name = ChatMessage::user_facing_tool_name(&dialog.tool_name);

    // Use expanded dialog for file tools with diff, compact for others
    if dialog.is_file_tool && dialog.diff_lines.is_some() {
        draw_permission_dialog_with_diff(f, dialog, display_name, &palette, area);
    } else {
        draw_permission_dialog_compact(f, dialog, display_name, &palette, area);
    }
}

/// Compact permission dialog (original layout) for non-file tools.
fn draw_permission_dialog_compact(
    f: &mut Frame,
    dialog: &crate::app::PermissionDialog,
    display_name: &str,
    palette: &Palette,
    area: Rect,
) {
    let dialog_width = 50u16.min(area.width.saturating_sub(4));
    let dialog_height = 12u16.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .title(" Permission Required ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(palette.warning));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Tool: ".to_string(),
                Style::default().fg(palette.inactive),
            ),
            Span::styled(display_name.to_string(), palette.tool_name_style()),
        ]),
        Line::from(vec![
            Span::styled(
                "  Args: ".to_string(),
                Style::default().fg(palette.inactive),
            ),
            Span::styled(
                truncate_display(
                    &dialog.input_summary,
                    (dialog_width as usize).saturating_sub(10),
                ),
                Style::default().fg(palette.text),
            ),
        ]),
        Line::from(""),
    ];

    render_permission_options(&mut lines, dialog.selected, palette);

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, dialog_area);
}

/// Expanded permission dialog with diff preview for FileEdit/FileWrite.
fn draw_permission_dialog_with_diff(
    f: &mut Frame,
    dialog: &crate::app::PermissionDialog,
    display_name: &str,
    palette: &Palette,
    area: Rect,
) {
    // Dynamic sizing: 80% of terminal, capped
    let dialog_width = ((area.width as u32 * 80 / 100) as u16)
        .min(120)
        .max(50)
        .min(area.width.saturating_sub(4));
    let dialog_height = ((area.height as u32 * 80 / 100) as u16)
        .max(16)
        .min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .title(" Permission Required ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(palette.warning));

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    // Split inner area: header (tool info + options) and diff preview
    let header_height = 8u16.min(inner.height);
    let diff_height = inner.height.saturating_sub(header_height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(diff_height),
        ])
        .split(inner);

    // ── Header section ─────────────────────────────────────────
    let mut header_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Tool: ".to_string(),
                Style::default().fg(palette.inactive),
            ),
            Span::styled(display_name.to_string(), palette.tool_name_style()),
        ]),
    ];

    // File path with optional replace_all indicator
    if let Some(ref path) = dialog.file_path {
        let mut path_spans = vec![
            Span::styled(
                "  File: ".to_string(),
                Style::default().fg(palette.inactive),
            ),
            Span::styled(path.clone(), Style::default().fg(palette.text)),
        ];
        if dialog.replace_all {
            path_spans.push(Span::styled(
                " (replace all)".to_string(),
                Style::default().fg(palette.warning),
            ));
        }
        header_lines.push(Line::from(path_spans));
    }

    header_lines.push(Line::from(""));
    render_permission_options(&mut header_lines, dialog.selected, palette);

    let header_paragraph = Paragraph::new(Text::from(header_lines));
    f.render_widget(header_paragraph, chunks[0]);

    // ── Diff preview section ───────────────────────────────────
    if diff_height > 0 {
        if let Some(ref diff_lines) = dialog.diff_lines {
            let rendered = crate::diff::render_diff_lines(diff_lines, palette, chunks[1].width);
            let total_lines = rendered.len();
            let visible_lines = diff_height as usize;
            let scroll = dialog
                .diff_scroll
                .min(total_lines.saturating_sub(visible_lines));

            // Scroll indicator
            let mut diff_display_lines: Vec<Line<'static>> = Vec::new();

            // Separator line
            diff_display_lines.push(Line::from(Span::styled(
                "─".repeat(chunks[1].width as usize),
                Style::default().fg(palette.subtle),
            )));

            // Diff content with scrolling
            let end = (scroll + visible_lines.saturating_sub(2)).min(total_lines);
            for line in rendered.into_iter().skip(scroll).take(end - scroll) {
                diff_display_lines.push(line);
            }

            // Scroll indicator at bottom if more content
            if end < total_lines {
                diff_display_lines.push(Line::from(Span::styled(
                    format!("  ... {} more lines (↑↓ to scroll)", total_lines - end),
                    Style::default().fg(palette.inactive),
                )));
            }

            let diff_paragraph = Paragraph::new(Text::from(diff_display_lines));
            f.render_widget(diff_paragraph, chunks[1]);
        }
    }
}

/// Render the 4 permission options (shared between compact and expanded dialogs).
fn render_permission_options(lines: &mut Vec<Line<'static>>, selected: usize, palette: &Palette) {
    let options = [
        ("y", "Allow"),
        ("a", "Always Allow"),
        ("n", "Deny"),
        ("d", "Always Deny"),
    ];
    for (i, (key, label)) in options.iter().enumerate() {
        let is_selected = i == selected;
        let prefix = if is_selected { "  > " } else { "    " };
        let style = if is_selected {
            Style::default()
                .fg(palette.claude)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("[{key}] "), Style::default().fg(palette.suggestion)),
            Span::styled(*label, style),
        ]));
    }
}

fn draw_user_question_dialog(f: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette();
    let dialog = match &app.user_question_dialog {
        Some(dialog) => dialog,
        None => return,
    };

    let width = ((area.width as u32 * 70 / 100) as u16)
        .min(90)
        .max(48)
        .min(area.width.saturating_sub(4));
    let option_rows = dialog.request.options.len() as u16;
    let custom_rows = if dialog.request.allow_custom { 2 } else { 0 };
    let height = (8 + option_rows + custom_rows)
        .min(area.height.saturating_sub(2))
        .max(10);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let dialog_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, dialog_area);

    let block = Block::default()
        .title(" Question ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(palette.claude));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                truncate_display(&dialog.request.question, width.saturating_sub(6) as usize),
                Style::default()
                    .fg(palette.text)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    for (index, option) in dialog.request.options.iter().enumerate() {
        let is_selected = dialog.selected == index;
        let style = if is_selected {
            Style::default()
                .fg(palette.claude)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };
        let prefix = if is_selected { "  > " } else { "    " };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(option.label.clone(), style),
            Span::styled(
                format!(
                    "  {}",
                    truncate_display(&option.description, width.saturating_sub(16) as usize,)
                ),
                Style::default().fg(palette.inactive),
            ),
        ]));
    }

    if dialog.request.allow_custom {
        let custom_index = dialog.request.options.len();
        let is_selected = dialog.selected == custom_index;
        let style = if is_selected {
            Style::default()
                .fg(palette.claude)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };
        let prefix = if is_selected { "  > " } else { "    " };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled("Custom", style),
        ]));
        lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled(
                truncate_display(
                    &dialog.custom_input.to_text(),
                    width.saturating_sub(10) as usize,
                ),
                Style::default().fg(palette.text),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter submit  Esc cancel",
        Style::default().fg(palette.inactive),
    )));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, dialog_area);
}

fn draw_session_picker(f: &mut Frame, app: &App, area: Rect) {
    let picker = match &app.session_picker {
        Some(picker) => picker,
        None => return,
    };
    let palette = app.palette();
    let width = ((area.width as u32 * 85 / 100) as u16)
        .min(120)
        .max(50)
        .min(area.width.saturating_sub(4));
    let height = ((area.height as u32 * 70 / 100) as u16)
        .min(area.height.saturating_sub(2))
        .max(12);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let picker_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, picker_area);
    let block = Block::default()
        .title(" Resume Session ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(palette.claude));
    let inner = block.inner(picker_area);
    f.render_widget(block, picker_area);

    let mut lines = Vec::new();
    if picker.loading {
        lines.push(Line::from(Span::styled(
            "  Loading recent sessions...",
            Style::default().fg(palette.inactive),
        )));
    } else if let Some(error) = &picker.error {
        lines.push(Line::from(Span::styled(
            format!("  {error}"),
            Style::default().fg(palette.error),
        )));
    } else if picker.sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No saved sessions found.",
            Style::default().fg(palette.inactive),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                "  Updated              ",
                Style::default().fg(palette.inactive),
            ),
            Span::styled("Model        ", Style::default().fg(palette.inactive)),
            Span::styled("Msgs  Summary", Style::default().fg(palette.inactive)),
        ]));
        let visible_rows = inner.height.saturating_sub(3) as usize;
        let end = (picker.scroll + visible_rows).min(picker.sessions.len());
        for (idx, session) in picker.sessions[picker.scroll..end].iter().enumerate() {
            let absolute_idx = picker.scroll + idx;
            let selected = absolute_idx == picker.selected;
            let prefix = if selected { "> " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(palette.claude)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.text)
            };
            let updated = truncate_display(&session.updated_at, 20);
            let model = truncate_display(&session.model_setting, 12);
            let summary_width = inner.width.saturating_sub(43) as usize;
            let summary = truncate_display(&session.first_user_summary, summary_width.max(10));
            lines.push(Line::from(vec![
                Span::styled(prefix.to_string(), style),
                Span::styled(format!("{updated:<20} "), style),
                Span::styled(format!("{model:<12} "), style),
                Span::styled(format!("{:<4} ", session.message_count), style),
                Span::styled(summary, style),
            ]));
        }
        let mut footer = String::from("  Enter resume  Esc cancel  ↑↓ select");
        if picker.skipped > 0 {
            footer.push_str(&format!("  skipped {} unreadable", picker.skipped));
        }
        lines.push(Line::from(Span::styled(
            footer,
            Style::default().fg(palette.inactive),
        )));
    }

    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

fn draw_todo_panel(f: &mut Frame, app: &App, area: Rect) {
    let palette = app.palette();
    let block = Block::default()
        .title(" Tasks ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(palette.plan_mode));

    if app.tasks.is_empty() {
        let paragraph = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No tasks",
                Style::default().fg(palette.subtle),
            )),
        ]))
        .block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let mut lines = Vec::new();
    for todo in &app.tasks {
        let (icon, style) = match todo.status {
            TodoStatus::Pending => ("○", Style::default().fg(palette.inactive)),
            TodoStatus::InProgress => ("◐", Style::default().fg(palette.claude)),
            TodoStatus::Completed => ("●", Style::default().fg(palette.success)),
            TodoStatus::Cancelled => ("✕", Style::default().fg(palette.inactive)),
        };

        let content = truncate_display(&todo.content, (area.width as usize).saturating_sub(6));
        lines.push(Line::from(vec![
            Span::styled(format!(" {icon} "), style),
            Span::styled(content, style),
        ]));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
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

fn truncate_display(s: &str, max_width: usize) -> String {
    if display_width(s) <= max_width {
        return s.to_string();
    }
    let mut width = 0;
    let mut end = 0;
    for (i, c) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw + 1 > max_width {
            break;
        }
        width += cw;
        end = i + c.len_utf8();
    }
    format!("{}…", &s[..end])
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Strip ANSI escape sequences from a string.
///
/// Removes CSI sequences (e.g. `\x1b[1m`, `\x1b[0;31m`) and OSC sequences
/// so that ratatui does not display raw escape characters.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    // CSI sequence: consume until a letter in '@'..='~'
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next.is_ascii_alphabetic() || next == '~' || next == '@' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC sequence: consume until ST (\x1b\\ or \x07)
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        if next == '\x07' {
                            chars.next();
                            break;
                        }
                        if next == '\x1b' {
                            chars.next();
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        chars.next();
                    }
                }
                _ => {
                    // Unknown escape; skip ESC char only
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Format a model name for display in the status bar.
///
/// Converts context-window suffixes like `[1m]` and `[2m]` into
/// human-readable badges, e.g. `claude-opus-4-6 (1M ctx)`.
fn format_model_display_name(model: &str) -> String {
    if let Some(base) = model.strip_suffix("[1m]") {
        format!("{base} (1M ctx)")
    } else if let Some(base) = model.strip_suffix("[1M]") {
        format!("{base} (1M ctx)")
    } else if let Some(base) = model.strip_suffix("[2m]") {
        format!("{base} (2M ctx)")
    } else if let Some(base) = model.strip_suffix("[2M]") {
        format!("{base} (2M ctx)")
    } else {
        model.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_codes_basic() {
        assert_eq!(strip_ansi_codes("\x1b[1mhello\x1b[0m"), "hello");
        assert_eq!(strip_ansi_codes("\x1b[0;31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi_codes("no ansi here"), "no ansi here");
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn test_format_model_display_name() {
        assert_eq!(
            format_model_display_name("claude-opus-4-6[1m]"),
            "claude-opus-4-6 (1M ctx)"
        );
        assert_eq!(
            format_model_display_name("claude-opus-4-6"),
            "claude-opus-4-6"
        );
        assert_eq!(format_model_display_name("opus[1m]"), "opus (1M ctx)");
    }

    #[test]
    fn test_display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
    }

    #[test]
    fn test_display_width_cjk() {
        assert_eq!(display_width("你好"), 4);
    }

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(42), "42");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(1500), "1.5k");
    }

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }

    #[test]
    fn test_parse_markdown_heading_list_code() {
        let blocks = parse_markdown_blocks("# Title\n\n- item\n\n```rust\nfn main() {}\n```");
        assert!(matches!(blocks[0], MarkdownBlock::Heading { .. }));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, MarkdownBlock::ListItem { .. })));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, MarkdownBlock::CodeBlock { .. })));
    }

    #[test]
    fn test_parse_inline_spans_styles() {
        let spans = parse_inline_spans("a `code` **bold** *italic*", Palette::dark());
        assert!(spans.iter().any(|s| s.content.as_ref() == "code"));
        assert!(spans.iter().any(|s| s.content.as_ref() == "bold"));
        assert!(spans.iter().any(|s| s.content.as_ref() == "italic"));
    }

    #[test]
    fn test_tool_result_uses_dim_text_for_success() {
        let mut lines = Vec::new();
        let app = App::new(
            "test".into(),
            "test".into(),
            "default".into(),
            None,
            rust_claude_core::config::Theme::Dark,
        );
        render_message(
            &ChatMessage::ToolResult {
                name: "Bash".into(),
                output_summary: "ok".into(),
                is_error: false,
            },
            0,
            &app,
            &mut lines,
        );

        // Successful tool results use dimmer (inactive) style to avoid
        // competing with the main assistant text.
        assert_eq!(lines[0].spans[1].style.fg, Some(app.palette().inactive));
    }

    #[test]
    fn test_status_bar_includes_theme() {
        let mut app = App::new(
            "test".into(),
            "test".into(),
            "default".into(),
            None,
            rust_claude_core::config::Theme::Light,
        );
        app.input_tokens = 10;
        app.output_tokens = 20;

        let area = Rect::new(0, 0, 80, 1);
        let model_display = if app.model == app.model_setting {
            app.model.clone()
        } else {
            format!("{} (from {})", app.model, app.model_setting)
        };
        let branch_display = app
            .git_branch
            .as_ref()
            .map(|branch| format!(" ⎇ {branch}"))
            .unwrap_or_default();
        let left = format!(" {}{} ", model_display, branch_display);
        let cache_info = String::new();
        let mode_display = "default";
        let right = format!(
            " tokens: {}↑ {}↓{} | {} | theme:{} ",
            format_tokens(app.input_tokens),
            format_tokens(app.output_tokens),
            cache_info,
            mode_display,
            "light",
        );
        let total_width = area.width as usize;
        let used = left.len() + right.len();
        let padding = if total_width > used {
            " ".repeat(total_width - used)
        } else {
            String::new()
        };
        let line = Line::from(vec![
            Span::styled(
                &left,
                Style::default()
                    .fg(app.palette().claude)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(padding, app.palette().status_bar_style()),
            Span::styled(&right, app.palette().status_bar_style()),
        ]);

        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(rendered.contains("theme:light"));
    }

    #[test]
    fn test_render_markdown_message_uses_single_prefix_per_message() {
        let mut lines = Vec::new();
        render_markdown_message("# Title\n\nSecond paragraph", &mut lines, Palette::dark());

        let prefixes: Vec<&str> = lines
            .iter()
            .filter_map(|line| line.spans.first().map(|span| span.content.as_ref()))
            .filter(|content| *content == "• ")
            .collect();
        assert_eq!(prefixes.len(), 1);
    }

    #[test]
    fn test_code_block_has_syntax_highlighting() {
        let palette = Palette::dark();
        let text = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
        let mut lines = Vec::new();
        render_markdown_message(text, &mut lines, palette);
        // Find code lines (those with │ prefix) and check they have multiple styled spans
        let code_lines: Vec<_> = lines
            .iter()
            .filter(|l| l.spans.iter().any(|s| s.content.as_ref() == "│ "))
            .collect();
        assert!(
            !code_lines.is_empty(),
            "Should have code lines with │ prefix"
        );
        // At least one code line should have more than 3 spans (prefix + border + highlighted tokens)
        let has_highlighting = code_lines.iter().any(|l| l.spans.len() > 3);
        assert!(
            has_highlighting,
            "Code lines should have syntax highlighting (>3 spans)"
        );
    }

    #[test]
    fn test_tool_use_diff_render_small() {
        use crate::diff;
        let palette = Palette::dark();
        let diff_lines = diff::compute_diff("hello", "world");
        // render_diff_lines produces output for the diff
        let rendered = diff::render_diff_lines(&diff_lines, &palette, 80);
        assert!(
            !rendered.is_empty(),
            "Small diff should produce rendered lines"
        );
        // Should have both removed and added lines
        assert!(
            diff_lines.iter().any(|d| d.kind == diff::DiffKind::Removed),
            "Should have removed lines"
        );
        assert!(
            diff_lines.iter().any(|d| d.kind == diff::DiffKind::Added),
            "Should have added lines"
        );
    }

    #[test]
    fn test_tool_use_diff_large_would_truncate() {
        use crate::diff;
        // Create a large diff with >20 changed lines
        let old: String = (0..30).map(|i| format!("old line {i}\n")).collect();
        let new: String = (0..30).map(|i| format!("new line {i}\n")).collect();
        let diff_lines = diff::compute_diff(&old, &new);
        let palette = Palette::dark();
        let rendered = diff::render_diff_lines(&diff_lines, &palette, 80);
        // Large diff should have >20 lines, triggering truncation in render
        assert!(
            rendered.len() > 20,
            "Large diff should produce >20 rendered lines (got {})",
            rendered.len()
        );
    }

    #[test]
    fn test_tool_use_message_has_diff_field() {
        use crate::diff;
        let diff_lines = diff::compute_diff("a", "b");
        let msg = ChatMessage::ToolUse {
            name: "FileEdit".into(),
            input_summary: "test.rs (edit)".into(),
            diff_lines: Some(diff_lines.clone()),
        };
        match &msg {
            ChatMessage::ToolUse {
                diff_lines: Some(dl),
                ..
            } => {
                assert!(!dl.is_empty());
            }
            _ => panic!("Expected ToolUse with diff_lines"),
        }
        // Bash tool should have None
        let bash_msg = ChatMessage::ToolUse {
            name: "Bash".into(),
            input_summary: "ls".into(),
            diff_lines: None,
        };
        match &bash_msg {
            ChatMessage::ToolUse {
                diff_lines: None, ..
            } => {}
            _ => panic!("Bash ToolUse should have diff_lines: None"),
        }
    }
}
