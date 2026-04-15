use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::events::ChatMessage;

/// Draw the entire TUI frame.
pub fn draw(f: &mut Frame, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.size());

    draw_status_bar(f, app, areas[0]);
    draw_chat_area(f, app, areas[1]);
    draw_input_area(f, app, areas[2]);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let spinner = if app.is_streaming { " ..." } else { "" };
    let status = format!(
        " model: {} | mode: {} | tokens: {}/{}{} ",
        app.model, app.permission_mode, app.input_tokens, app.output_tokens, spinner
    );

    let paragraph = Paragraph::new(status)
        .style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(paragraph, area);
}

fn draw_chat_area(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for message in &app.messages {
        lines.extend(render_message(message));
    }

    if app.is_streaming && !app.streaming_text.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "Claude: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.streaming_text),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No messages yet. Type a prompt below.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Chat "))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    f.render_widget(paragraph, area);
}

fn draw_input_area(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title = if app.is_streaming {
        " Input (Streaming...) "
    } else {
        " Input "
    };

    let paragraph = Paragraph::new(app.input.clone())
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .style(if app.is_streaming {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        });

    f.render_widget(paragraph, area);

    // Show cursor only while not streaming.
    if !app.is_streaming {
        let x = area.x + 1 + display_width(&app.input[..app.input_cursor]) as u16;
        let y = area.y + 1;
        f.set_cursor(x.min(area.right().saturating_sub(1)), y);
    }
}

fn render_message(message: &ChatMessage) -> Vec<Line<'static>> {
    match message {
        ChatMessage::User(text) => vec![Line::from(vec![
            Span::styled(
                "You: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(text.clone()),
        ])],
        ChatMessage::Assistant(text) => vec![Line::from(vec![
            Span::styled("Claude: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(text.clone()),
        ])],
        ChatMessage::ToolUse { name, input_summary } => vec![Line::from(vec![
            Span::styled(
                format!("Tool: {name}"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(" "),
            Span::styled(input_summary.clone(), Style::default().fg(Color::Gray)),
        ])],
        ChatMessage::ToolResult {
            name,
            output_summary,
            is_error,
        } => vec![Line::from(vec![
            Span::styled(
                if *is_error {
                    format!("Error: {name}")
                } else {
                    format!("Result: {name}")
                },
                Style::default().fg(if *is_error { Color::Red } else { Color::Green }),
            ),
            Span::raw(" "),
            Span::raw(output_summary.clone()),
        ])],
        ChatMessage::System(text) => vec![Line::from(vec![
            Span::styled("System: ", Style::default().fg(Color::Yellow)),
            Span::raw(text.clone()),
        ])],
    }
}

fn display_width(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
    }

    #[test]
    fn test_render_user_message() {
        let lines = render_message(&ChatMessage::User("hello".into()));
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[1].content.as_ref(), "hello");
    }

    #[test]
    fn test_render_tool_result_message() {
        let lines = render_message(&ChatMessage::ToolResult {
            name: "Bash".into(),
            output_summary: "ok".into(),
            is_error: false,
        });
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans[0].content.contains("Result: Bash"));
    }
}
