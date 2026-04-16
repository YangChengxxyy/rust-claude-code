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
use crate::theme;

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
    let mut lines: Vec<Line> = Vec::new();

    for (idx, msg) in app.messages.iter().enumerate() {
        render_message(msg, idx, app, &mut lines);
    }

    if app.is_streaming && !app.streaming_text.is_empty() {
        for (i, line) in app.streaming_text.lines().enumerate() {
            if i == 0 {
                let line_text = line.to_owned();
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", theme::BLACK_CIRCLE), theme::bullet_style()),
                    Span::styled(line_text, theme::assistant_text_style()),
                ]));
            } else {
                let line_text = line.to_owned();
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line_text, theme::assistant_text_style()),
                ]));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Type a message below to get started.",
            Style::default().fg(theme::SUBTLE),
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
    let width: usize = line.spans.iter().map(|span| display_width(span.content.as_ref())).sum();
    width.max(1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownBlock {
    Heading { level: usize, text: String },
    ListItem { ordered: bool, marker: String, text: String },
    CodeBlock { language: Option<String>, code: String },
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
                                .fg(theme::SUGGESTION)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(line.to_string(), theme::user_text_style()),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(line.to_string(), theme::user_text_style()),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }
        ChatMessage::Assistant(text) => {
            render_markdown_message(text, lines);
        }
        ChatMessage::Thinking { summary, content } => {
            let is_selected = app.selected_thinking == Some(message_index);
            let is_expanded = app.expanded_thinking.contains(&message_index);
            let prefix = if is_selected { ">" } else { " " };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {prefix} "),
                    if is_selected {
                        Style::default().fg(theme::CLAUDE).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::SUBTLE)
                    },
                ),
                Span::styled("Thinking", theme::spinner_style().add_modifier(Modifier::BOLD)),
                Span::raw(" — "),
                Span::styled(summary.clone(), Style::default().fg(theme::INACTIVE)),
                Span::styled(
                    if is_expanded { " [Tab to collapse]" } else { " [Tab to expand]" },
                    Style::default().fg(theme::SUBTLE),
                ),
            ]));
            if is_expanded {
                for line in content.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(line.to_string(), Style::default().fg(theme::INACTIVE)),
                    ]));
                }
            }
        }
        ChatMessage::ToolUse { name, input_summary } => {
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
                        theme::response_prefix_style(),
                    ),
                    Span::styled(format!("{display_name} "), theme::tool_name_style()),
                    Span::styled("! ", theme::bash_prefix_style()),
                    Span::styled(cmd_display, theme::bash_command_style()),
                ]));
            } else {
                let mut spans = vec![
                    Span::styled(
                        format!("  {} ", theme::RESPONSE_PREFIX),
                        theme::response_prefix_style(),
                    ),
                    Span::styled(display_name.to_string(), theme::tool_name_style()),
                ];
                if !input_summary.is_empty() {
                    spans.push(Span::styled(
                        format!(" ({input_summary})"),
                        theme::tool_desc_style(),
                    ));
                }
                lines.push(Line::from(spans));
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
                theme::error_style()
            } else {
                Style::default().fg(theme::INACTIVE)
            };

            for (i, line) in output_summary.lines().take(6).enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {} ", theme::RESPONSE_PREFIX),
                            theme::response_prefix_style(),
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
                        Style::default().fg(theme::SUBTLE),
                    ),
                ]));
            }
        }
        ChatMessage::System(text) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(text.clone(), theme::warning_style()),
            ]));
        }
    }
}

fn render_markdown_message(text: &str, lines: &mut Vec<Line<'static>>) {
    let blocks = parse_markdown_blocks(text);
    let mut first_block = true;
    for block in blocks {
        match block {
            MarkdownBlock::Heading { level, text } => {
                if !first_block {
                    lines.push(Line::from(""));
                }
                let style = match level {
                    1 => Style::default().fg(theme::CLAUDE).add_modifier(Modifier::BOLD),
                    2 => Style::default().fg(theme::SUGGESTION).add_modifier(Modifier::BOLD),
                    _ => Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", theme::BLACK_CIRCLE), theme::bullet_style()),
                    Span::styled(text, style),
                ]));
            }
            MarkdownBlock::ListItem { ordered: _, marker, text } => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{marker} "), Style::default().fg(theme::SUGGESTION)),
                    ]));
                if let Some(last) = lines.last_mut() {
                    last.spans.extend(parse_inline_spans(&text));
                }
            }
            MarkdownBlock::CodeBlock { language, code } => {
                if !first_block {
                    lines.push(Line::from(""));
                }
                let title = language.unwrap_or_else(|| "code".to_string());
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("┌─ {title}"), Style::default().fg(theme::BASH_BORDER)),
                ]));
                for line in code.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("│ ", Style::default().fg(theme::BASH_BORDER)),
                        Span::styled(line.to_string(), Style::default().fg(theme::TEXT)),
                    ]));
                }
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("└─", Style::default().fg(theme::BASH_BORDER)),
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
                        spans.push(Span::styled(
                            format!("{} ", theme::BLACK_CIRCLE),
                            theme::bullet_style(),
                        ));
                        first_line = false;
                    } else {
                        spans.push(Span::raw("  "));
                    }
                    spans.extend(parse_inline_spans(logical_line));
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
        if (1..=3).contains(&heading_level)
            && trimmed.chars().nth(heading_level) == Some(' ')
        {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(MarkdownBlock::Heading {
                level: heading_level,
                text: trimmed[heading_level + 1..].to_string(),
            });
            continue;
        }

        if let Some(text) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
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

fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut plain = String::new();

    while i < chars.len() {
        if chars[i] == '`' {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), theme::assistant_text_style()));
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '`' {
                j += 1;
            }
            if j < chars.len() {
                let content: String = chars[i + 1..j].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default()
                        .fg(theme::TEXT)
                        .bg(theme::USER_MSG_BG),
                ));
                i = j + 1;
                continue;
            }
        }

        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), theme::assistant_text_style()));
            }
            let mut j = i + 2;
            while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '*') {
                j += 1;
            }
            if j + 1 < chars.len() {
                let content: String = chars[i + 2..j].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
                ));
                i = j + 2;
                continue;
            }
        }

        if chars[i] == '*' {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), theme::assistant_text_style()));
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '*' {
                j += 1;
            }
            if j < chars.len() {
                let content: String = chars[i + 1..j].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().fg(theme::TEXT).add_modifier(Modifier::ITALIC),
                ));
                i = j + 1;
                continue;
            }
        }

        plain.push(chars[i]);
        i += 1;
    }

    if !plain.is_empty() {
        spans.push(Span::styled(plain, theme::assistant_text_style()));
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }
    spans
}

fn draw_spinner_line(f: &mut Frame, app: &App, area: Rect) {
    let content = if app.is_thinking {
        Line::from(vec![
            Span::styled(format!("{} ", theme::BLACK_CIRCLE), theme::spinner_style()),
            Span::styled("Thinking…", theme::spinner_style()),
        ])
    } else if app.is_streaming {
        Line::from(vec![
            Span::styled(format!("{} ", theme::BLACK_CIRCLE), theme::spinner_style()),
            Span::styled("Streaming…", theme::spinner_style()),
        ])
    } else {
        Line::from("")
    };

    f.render_widget(Paragraph::new(content), area);
}

fn draw_input_area(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.is_streaming {
        Style::default().fg(theme::SUBTLE)
    } else {
        theme::prompt_border_style()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(border_style)
        .title(if app.is_streaming { " Input (locked) " } else { " Input " });

    let input_style = if app.is_streaming {
        theme::input_disabled_style()
    } else {
        theme::user_text_style()
    };

    let input_text = build_input_text(app, input_style);
    let paragraph = Paragraph::new(input_text)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);

    if !app.is_streaming {
        let CursorPosition { row, col } = app.input_cursor();
        let current_line = app
            .input_text()
            .lines()
            .nth(row)
            .unwrap_or_default()
            .to_string();
        let cursor_x = area.x + 1 + 2 + display_width(&current_line[..char_to_byte(&current_line, col)]) as u16;
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
        return Text::from(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme::SUGGESTION)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let mut lines = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            let line_text = line.to_owned();
            lines.push(Line::from(vec![
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(theme::SUGGESTION)
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
    let mode_display = match app.permission_mode.as_str() {
        "BypassPermissions" => "bypass",
        "AcceptEdits" => "accept-edits",
        "DontAsk" => "dont-ask",
        "Plan" => "plan",
        "Default" => "default",
        other => other,
    };

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
    let cache_info = if app.input_tokens > 0 {
        let hit_pct = (app.cache_read_input_tokens as f64 / app.input_tokens as f64 * 100.0) as u32;
        format!(" cache:{hit_pct}%")
    } else {
        String::new()
    };

    let right = format!(
        " tokens: {}↑ {}↓{} | {} ",
        format_tokens(app.input_tokens),
        format_tokens(app.output_tokens),
        cache_info,
        mode_display,
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
            Style::default().fg(theme::CLAUDE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(padding, theme::status_bar_style()),
        Span::styled(&right, theme::status_bar_style()),
    ]);

    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

fn draw_permission_dialog(f: &mut Frame, app: &App, area: Rect) {
    let dialog = match &app.permission_dialog {
        Some(d) => d,
        None => return,
    };

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
        .border_style(Style::default().fg(theme::WARNING));

    let display_name = ChatMessage::user_facing_tool_name(&dialog.tool_name);

    let options = [
        ("y", "Allow", "Allow this operation"),
        ("a", "Always Allow", "Add to always-allow rules"),
        ("n", "Deny", "Deny this operation"),
        ("d", "Always Deny", "Add to always-deny rules"),
    ];

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tool: ", Style::default().fg(theme::INACTIVE)),
            Span::styled(display_name, theme::tool_name_style()),
        ]),
        Line::from(vec![
            Span::styled("  Args: ", Style::default().fg(theme::INACTIVE)),
            Span::styled(
                truncate_display(&dialog.input_summary, (dialog_width as usize).saturating_sub(10)),
                Style::default().fg(theme::TEXT),
            ),
        ]),
        Line::from(""),
    ];

    for (i, (key, label, _desc)) in options.iter().enumerate() {
        let is_selected = i == dialog.selected;
        let prefix = if is_selected { "  > " } else { "    " };
        let style = if is_selected {
            Style::default()
                .fg(theme::CLAUDE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::TEXT)
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("[{key}] "), Style::default().fg(theme::SUGGESTION)),
            Span::styled(*label, style),
        ]));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, dialog_area);
}

fn draw_todo_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Todo ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(theme::PLAN_MODE));

    if app.todos.is_empty() {
        let paragraph = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No tasks",
                Style::default().fg(theme::SUBTLE),
            )),
        ]))
        .block(block);
        f.render_widget(paragraph, area);
        return;
    }

    let mut lines = Vec::new();
    for todo in &app.todos {
        let (icon, style) = match todo.status {
            TodoStatus::Pending => ("○", Style::default().fg(theme::INACTIVE)),
            TodoStatus::InProgress => ("◐", Style::default().fg(theme::CLAUDE)),
            TodoStatus::Completed => ("●", Style::default().fg(theme::SUCCESS)),
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(blocks.iter().any(|b| matches!(b, MarkdownBlock::ListItem { .. })));
        assert!(blocks.iter().any(|b| matches!(b, MarkdownBlock::CodeBlock { .. })));
    }

    #[test]
    fn test_parse_inline_spans_styles() {
        let spans = parse_inline_spans("a `code` **bold** *italic*");
        assert!(spans.iter().any(|s| s.content.as_ref() == "code"));
        assert!(spans.iter().any(|s| s.content.as_ref() == "bold"));
        assert!(spans.iter().any(|s| s.content.as_ref() == "italic"));
    }

    #[test]
    fn test_render_thinking_summary() {
        let mut lines = Vec::new();
        let mut app = App::new("test".into(), "test".into(), "default".into(), None);
        app.selected_thinking = Some(0);
        render_message(
            &ChatMessage::Thinking {
                summary: "Thought for ~10 chars".into(),
                content: "reasoning".into(),
            },
            0,
            &app,
            &mut lines,
        );
        assert!(lines[0].spans.iter().any(|s| s.content.contains("Thinking")));
    }
}
