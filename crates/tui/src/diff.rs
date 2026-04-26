//! Diff computation and rendering module using the `similar` crate.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::Palette;

/// Classification of a diff line.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffKind {
    Added,
    Removed,
    Context,
}

/// A single line in a diff view, with optional old/new line numbers.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub content: String,
    pub old_lineno: Option<usize>,
    pub new_lineno: Option<usize>,
}

/// Compute a line-level diff between `old` and `new` strings.
///
/// Uses `similar::TextDiff::from_lines` to produce a unified-style
/// list of `DiffLine` values with correct old/new line numbers.
pub fn compute_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut result = Vec::new();
    let mut old_lineno = 1usize;
    let mut new_lineno = 1usize;

    for change in diff.iter_all_changes() {
        let kind = match change.tag() {
            similar::ChangeTag::Delete => DiffKind::Removed,
            similar::ChangeTag::Insert => DiffKind::Added,
            similar::ChangeTag::Equal => DiffKind::Context,
        };

        let content = change.value().strip_suffix('\n').unwrap_or(change.value());
        let content = content.to_string();

        let line = match kind {
            DiffKind::Removed => {
                let line = DiffLine {
                    kind,
                    content,
                    old_lineno: Some(old_lineno),
                    new_lineno: None,
                };
                old_lineno += 1;
                line
            }
            DiffKind::Added => {
                let line = DiffLine {
                    kind,
                    content,
                    old_lineno: None,
                    new_lineno: Some(new_lineno),
                };
                new_lineno += 1;
                line
            }
            DiffKind::Context => {
                let line = DiffLine {
                    kind,
                    content,
                    old_lineno: Some(old_lineno),
                    new_lineno: Some(new_lineno),
                };
                old_lineno += 1;
                new_lineno += 1;
                line
            }
        };

        result.push(line);
    }

    result
}

/// Render a slice of `DiffLine` values into ratatui `Line`s.
///
/// Each line is prefixed with a marker (`-`, `+`, or ` `) and old/new
/// line numbers in a gutter.  The background color is set according to
/// the diff kind via the provided `Palette`.
pub fn render_diff_lines(
    diff_lines: &[DiffLine],
    palette: &Palette,
    _width: u16,
) -> Vec<Line<'static>> {
    let mut max_old = 0usize;
    let mut max_new = 0usize;
    for dl in diff_lines {
        if let Some(n) = dl.old_lineno {
            if n > max_old {
                max_old = n;
            }
        }
        if let Some(n) = dl.new_lineno {
            if n > max_new {
                max_new = n;
            }
        }
    }
    let old_width = max_old.to_string().len().max(3);
    let new_width = max_new.to_string().len().max(3);

    diff_lines
        .iter()
        .map(|dl| {
            let (marker, bg) = match dl.kind {
                DiffKind::Added => ("+", palette.diff_added),
                DiffKind::Removed => ("-", palette.diff_removed),
                DiffKind::Context => (" ", Color::Reset),
            };

            let old_str = dl
                .old_lineno
                .map(|n| format!("{:>width$}", n, width = old_width))
                .unwrap_or_else(|| " ".repeat(old_width));
            let new_str = dl
                .new_lineno
                .map(|n| format!("{:>width$}", n, width = new_width))
                .unwrap_or_else(|| " ".repeat(new_width));

            let gutter = format!("{} {} | {} | ", marker, old_str, new_str);
            let style = Style::default().bg(bg);

            Line::from(vec![
                Span::styled(gutter, style),
                Span::styled(dl.content.clone(), style),
            ])
        })
        .collect()
}

/// Render a file-preview for a newly-created file.
///
/// All lines are shown as additions (green `+` prefix).  If the file
/// exceeds `max_lines`, the output is truncated and a trailing
/// indicator line is appended.
pub fn render_file_preview(
    content: &str,
    palette: &Palette,
    max_lines: usize,
    _width: u16,
) -> Vec<Line<'static>> {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let show = lines.len().min(max_lines);
    let new_width = total.to_string().len().max(3);

    let mut result: Vec<Line<'static>> = lines
        .iter()
        .take(show)
        .enumerate()
        .map(|(idx, text)| {
            let lineno = idx + 1;
            let gutter = format!("+ {:>width$} | ", lineno, width = new_width);
            let style = Style::default().bg(palette.diff_added);
            Line::from(vec![
                Span::styled(gutter, style),
                Span::styled((*text).to_string(), style),
            ])
        })
        .collect();

    if total > show {
        let indicator = format!("... ({} more lines)", total - show);
        result.push(Line::from(vec![Span::styled(
            indicator,
            Style::default().fg(palette.subtle),
        )]));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn test_palette() -> Palette {
        Palette::dark()
    }

    #[test]
    fn test_single_line_replacement() {
        let old = "hello world";
        let new = "hello rust";
        let diff = compute_diff(old, new);
        // Should produce 1 removed + 1 added (no context because entire line changed)
        assert_eq!(diff.len(), 2);
        assert_eq!(diff[0].kind, DiffKind::Removed);
        assert_eq!(diff[0].content, "hello world");
        assert_eq!(diff[0].old_lineno, Some(1));
        assert_eq!(diff[0].new_lineno, None);

        assert_eq!(diff[1].kind, DiffKind::Added);
        assert_eq!(diff[1].content, "hello rust");
        assert_eq!(diff[1].old_lineno, None);
        assert_eq!(diff[1].new_lineno, Some(1));
    }

    #[test]
    fn test_multi_line_addition() {
        let old = "line1\nline2";
        let new = "line1\ninserted\nline2";
        let diff = compute_diff(old, new);
        // line1 (context), inserted (added), line2 (context)
        assert_eq!(diff.len(), 3);
        assert_eq!(diff[0].kind, DiffKind::Context);
        assert_eq!(diff[0].content, "line1");
        assert_eq!(diff[0].old_lineno, Some(1));
        assert_eq!(diff[0].new_lineno, Some(1));

        assert_eq!(diff[1].kind, DiffKind::Added);
        assert_eq!(diff[1].content, "inserted");
        assert_eq!(diff[1].old_lineno, None);
        assert_eq!(diff[1].new_lineno, Some(2));

        assert_eq!(diff[2].kind, DiffKind::Context);
        assert_eq!(diff[2].content, "line2");
        assert_eq!(diff[2].old_lineno, Some(2));
        assert_eq!(diff[2].new_lineno, Some(3));
    }

    #[test]
    fn test_empty_old_string() {
        let old = "";
        let new = "new content\nline 2";
        let diff = compute_diff(old, new);
        assert_eq!(diff.len(), 2);
        assert!(diff.iter().all(|d| d.kind == DiffKind::Added));
        assert_eq!(diff[0].content, "new content");
        assert_eq!(diff[0].new_lineno, Some(1));
        assert_eq!(diff[1].content, "line 2");
        assert_eq!(diff[1].new_lineno, Some(2));
    }

    #[test]
    fn test_line_number_correctness() {
        let old = "a\nb\nc";
        let new = "a\nX\nb\nc";
        let diff = compute_diff(old, new);
        // a (context), X (added), b (context), c (context)
        assert_eq!(diff.len(), 4);
        assert_eq!(diff[0].old_lineno, Some(1));
        assert_eq!(diff[0].new_lineno, Some(1));
        assert_eq!(diff[1].new_lineno, Some(2));
        assert_eq!(diff[2].old_lineno, Some(2));
        assert_eq!(diff[2].new_lineno, Some(3));
        assert_eq!(diff[3].old_lineno, Some(3));
        assert_eq!(diff[3].new_lineno, Some(4));
    }

    #[test]
    fn test_render_diff_lines_produces_output() {
        let diff = compute_diff("foo\nbar", "foo\nbaz");
        let palette = test_palette();
        let lines = render_diff_lines(&diff, &palette, 80);
        assert!(!lines.is_empty());
        // Each line should contain at least one span
        for line in &lines {
            assert!(!line.spans.is_empty());
        }
    }

    #[test]
    fn test_render_file_preview_truncates() {
        let content = "1\n2\n3\n4\n5";
        let palette = test_palette();
        let lines = render_file_preview(content, &palette, 3, 80);
        // 3 content lines + 1 truncation indicator
        assert_eq!(lines.len(), 4);
        let last = lines.last().unwrap();
        let text: String = last.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("2 more lines"));
    }

    #[test]
    fn test_render_file_preview_no_truncation_when_under_limit() {
        let content = "1\n2";
        let palette = test_palette();
        let lines = render_file_preview(content, &palette, 5, 80);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_render_diff_lines_styles() {
        let diff = vec![
            DiffLine {
                kind: DiffKind::Added,
                content: "added line".into(),
                old_lineno: None,
                new_lineno: Some(1),
            },
            DiffLine {
                kind: DiffKind::Removed,
                content: "removed line".into(),
                old_lineno: Some(1),
                new_lineno: None,
            },
            DiffLine {
                kind: DiffKind::Context,
                content: "context line".into(),
                old_lineno: Some(2),
                new_lineno: Some(2),
            },
        ];
        let palette = test_palette();
        let lines = render_diff_lines(&diff, &palette, 80);
        assert_eq!(lines.len(), 3);

        // Added line background
        let added_bg = lines[0].spans[0].style.bg;
        assert_eq!(added_bg, Some(palette.diff_added));

        // Removed line background
        let removed_bg = lines[1].spans[0].style.bg;
        assert_eq!(removed_bg, Some(palette.diff_removed));

        // Context line background should be Reset (default)
        let context_bg = lines[2].spans[0].style.bg;
        assert_eq!(context_bg, Some(Color::Reset));
    }
}
