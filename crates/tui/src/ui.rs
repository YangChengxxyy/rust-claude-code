//! Rendering layer — draws the TUI frame in a style that matches Claude Code.
//!
//! Layout (top → bottom):
//!   1. Chat area   — scrollable message history + live streaming text
//!   2. Spinner row — one line showing "Thinking…" or streaming indicator
//!   3. Input area  — bordered prompt with `>` prefix
//!   4. Status line — model | tokens | mode
//!
//! Optional:
//!   - Todo panel (right side, toggled by Tab)
//!   - Permission dialog (centered modal overlay)

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

use crate::app::App;
use crate::events::ChatMessage;
use crate::theme;

/// Draw the entire TUI frame.
pub fn draw(f: &mut Frame, app: &App) {
    let full = f.size();

    // If the todo panel is visible, split horizontally: [main | todo]
    let (main_area, todo_area) = if app.todo_visible {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(30)])
            .split(full);
        (chunks[0], Some(chunks[1]))
    } else {
        (full, None)
    };

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // chat area
            Constraint::Length(1), // spinner / activity line
            Constraint::Length(3), // input area
            Constraint::Length(1), // status bar
        ])
        .split(main_area);

    draw_chat_area(f, app, areas[0]);
    draw_spinner_line(f, app, areas[1]);
    draw_input_area(f, app, areas[2]);
    draw_status_bar(f, app, areas[3]);

    if let Some(todo_area) = todo_area {
        draw_todo_panel(f, app, todo_area);
    }

    // Permission dialog is rendered last (on top of everything).
    if app.permission_dialog.is_some() {
        draw_permission_dialog(f, app, full);
    }
}

// ── Chat area ───────────────────────────────────────────────────────────────

fn draw_chat_area(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        render_message(msg, &mut lines, area.width as usize);
    }

    // Live streaming text
    if app.is_streaming && !app.streaming_text.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", theme::BLACK_CIRCLE),
                theme::bullet_style(),
            ),
            Span::styled(app.streaming_text.clone(), theme::assistant_text_style()),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Type a message below to get started.",
            Style::default().fg(theme::SUBTLE),
        )));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    f.render_widget(paragraph, area);
}

// ── Message rendering ───────────────────────────────────────────────────────

fn render_message(msg: &ChatMessage, lines: &mut Vec<Line<'static>>, _width: usize) {
    match msg {
        ChatMessage::User(text) => {
            // Blank separator line before user messages (unless first message).
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            // User messages: "> text" style
            for (i, line) in text.lines().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled("> ", Style::default().fg(theme::SUGGESTION).add_modifier(Modifier::BOLD)),
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
            // Assistant messages: "⏺ text"
            for (i, line) in text.lines().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{} ", theme::BLACK_CIRCLE),
                            theme::bullet_style(),
                        ),
                        Span::styled(line.to_string(), theme::assistant_text_style()),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(line.to_string(), theme::assistant_text_style()),
                    ]));
                }
            }
        }

        ChatMessage::ToolUse { name, input_summary } => {
            let display_name = ChatMessage::user_facing_tool_name(name);

            if name == "Bash" {
                // Bash tool: "  ⎿  ! command"
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
                    Span::styled(
                        format!("{display_name} "),
                        theme::tool_name_style(),
                    ),
                    Span::styled("! ", theme::bash_prefix_style()),
                    Span::styled(cmd_display, theme::bash_command_style()),
                ]));
            } else {
                // Other tools: "  ⎿  ToolName (summary)"
                let mut spans = vec![
                    Span::styled(
                        format!("  {} ", theme::RESPONSE_PREFIX),
                        theme::response_prefix_style(),
                    ),
                    Span::styled(
                        display_name.to_string(),
                        theme::tool_name_style(),
                    ),
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
            // Tool results: "  ⎿  output" (indented under the tool call)
            let style = if *is_error {
                theme::error_style()
            } else {
                Style::default().fg(theme::INACTIVE)
            };

            // Show first few lines of result, indented
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

// ── Spinner / activity line ─────────────────────────────────────────────────

fn draw_spinner_line(f: &mut Frame, app: &App, area: Rect) {
    let content = if app.is_thinking {
        Line::from(vec![
            Span::styled(
                format!("{} ", theme::BLACK_CIRCLE),
                theme::spinner_style(),
            ),
            Span::styled("Thinking…", theme::spinner_style()),
        ])
    } else if app.is_streaming {
        Line::from(vec![
            Span::styled(
                format!("{} ", theme::BLACK_CIRCLE),
                theme::spinner_style(),
            ),
            Span::styled("Streaming…", theme::spinner_style()),
        ])
    } else {
        Line::from("")
    };

    f.render_widget(Paragraph::new(content), area);
}

// ── Input area ──────────────────────────────────────────────────────────────

fn draw_input_area(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.is_streaming {
        Style::default().fg(theme::SUBTLE)
    } else {
        theme::prompt_border_style()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(border_style);

    // Build input text with ">" prompt prefix
    let input_style = if app.is_streaming {
        theme::input_disabled_style()
    } else {
        theme::user_text_style()
    };

    let input_text = if app.input.is_empty() && !app.is_streaming {
        Text::from(Line::from(vec![
            Span::styled("> ", Style::default().fg(theme::SUGGESTION).add_modifier(Modifier::BOLD)),
        ]))
    } else {
        Text::from(Line::from(vec![
            Span::styled("> ", Style::default().fg(theme::SUGGESTION).add_modifier(Modifier::BOLD)),
            Span::styled(app.input.clone(), input_style),
        ]))
    };

    let paragraph = Paragraph::new(input_text)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);

    // Show cursor only while not streaming.
    if !app.is_streaming {
        // +1 for border, +2 for "> " prefix
        let cursor_x = area.x + 1 + 2 + display_width(&app.input[..app.input_cursor]) as u16;
        let cursor_y = area.y + 1;
        f.set_cursor(cursor_x.min(area.right().saturating_sub(1)), cursor_y);
    }
}

// ── Status bar ──────────────────────────────────────────────────────────────

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

    let left = format!(" {} ", model_display);
    let right = format!(
        " tokens: {}↑ {}↓ | {} ",
        format_tokens(app.input_tokens),
        format_tokens(app.output_tokens),
        mode_display,
    );

    // Pad middle to fill the line
    let total_width = area.width as usize;
    let used = left.len() + right.len();
    let padding = if total_width > used {
        " ".repeat(total_width - used)
    } else {
        String::new()
    };

    let line = Line::from(vec![
        Span::styled(&left, Style::default().fg(theme::CLAUDE).add_modifier(Modifier::BOLD)),
        Span::styled(padding, theme::status_bar_style()),
        Span::styled(&right, theme::status_bar_style()),
    ]);

    // Thin separator at the top of status bar
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

// ── Permission dialog ──────────────────────────────────────────────────────

fn draw_permission_dialog(f: &mut Frame, app: &App, area: Rect) {
    let dialog = match &app.permission_dialog {
        Some(d) => d,
        None => return,
    };

    // Center the dialog: 50 wide, 12 tall
    let dialog_width = 50u16.min(area.width.saturating_sub(4));
    let dialog_height = 12u16.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    // Clear the area behind the dialog
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

// ── Todo panel ─────────────────────────────────────────────────────────────

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

// ── Helpers ─────────────────────────────────────────────────────────────────

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate a string to fit within a display width, appending "…" if needed.
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
        // CJK characters are double-width
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
    fn test_render_user_message() {
        let mut lines = Vec::new();
        render_message(&ChatMessage::User("hello".into()), &mut lines, 80);
        assert!(lines.len() >= 1);
        // First user message has no leading blank line
        let first_line = &lines[0];
        assert_eq!(first_line.spans[0].content.as_ref(), "> ");
        assert_eq!(first_line.spans[1].content.as_ref(), "hello");
    }

    #[test]
    fn test_render_assistant_message() {
        let mut lines = Vec::new();
        render_message(&ChatMessage::Assistant("hi there".into()), &mut lines, 80);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert!(line.spans[0].content.contains(theme::BLACK_CIRCLE));
        assert_eq!(line.spans[1].content.as_ref(), "hi there");
    }

    #[test]
    fn test_render_tool_use_bash() {
        let mut lines = Vec::new();
        render_message(
            &ChatMessage::ToolUse {
                name: "Bash".into(),
                input_summary: "ls -la".into(),
            },
            &mut lines,
            80,
        );
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        // Should contain the response prefix and bash prefix
        assert!(line.spans[0].content.contains(theme::RESPONSE_PREFIX));
        assert!(line.spans.iter().any(|s| s.content.contains("!")));
    }

    #[test]
    fn test_render_tool_use_file_read() {
        let mut lines = Vec::new();
        render_message(
            &ChatMessage::ToolUse {
                name: "FileRead".into(),
                input_summary: "/tmp/test.rs".into(),
            },
            &mut lines,
            80,
        );
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert!(line.spans.iter().any(|s| s.content.as_ref() == "Read"));
        assert!(line
            .spans
            .iter()
            .any(|s| s.content.contains("/tmp/test.rs")));
    }

    #[test]
    fn test_render_tool_result_ok() {
        let mut lines = Vec::new();
        render_message(
            &ChatMessage::ToolResult {
                name: "Bash".into(),
                output_summary: "file1\nfile2".into(),
                is_error: false,
            },
            &mut lines,
            80,
        );
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_render_tool_result_error() {
        let mut lines = Vec::new();
        render_message(
            &ChatMessage::ToolResult {
                name: "Bash".into(),
                output_summary: "command not found".into(),
                is_error: true,
            },
            &mut lines,
            80,
        );
        assert_eq!(lines.len(), 1);
        // Error style should have error color
        let style = lines[0].spans[1].style;
        assert_eq!(style.fg, Some(theme::ERROR));
    }


    #[test]
    fn test_status_bar_shows_model_source_relation() {
        let app = App::new(
            "claude-opus-4-6".into(),
            "opusplan".into(),
            "Plan".into(),
        );
        let model_display = if app.model == app.model_setting {
            app.model.clone()
        } else {
            format!("{} (from {})", app.model, app.model_setting)
        };
        assert_eq!(model_display, "claude-opus-4-6 (from opusplan)");
    }
}
