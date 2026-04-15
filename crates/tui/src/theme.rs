//! Centralized color theme matching Claude Code's dark theme.
//!
//! All RGB values are taken from the official Claude Code `darkTheme` in
//! `src/utils/theme.ts`. This gives us a single source of truth for colors
//! across the entire TUI rendering layer.

use ratatui::style::{Color, Modifier, Style};

// ── Core brand ──────────────────────────────────────────────────────────────

/// Claude orange — used for the spinner and brand indicator.
pub const CLAUDE: Color = Color::Rgb(215, 119, 87);

// ── Text ────────────────────────────────────────────────────────────────────

/// Primary text color (white on dark background).
pub const TEXT: Color = Color::Rgb(255, 255, 255);

/// Dimmed / subtle text (dark gray).
pub const SUBTLE: Color = Color::Rgb(80, 80, 80);

/// Inactive / secondary text (light gray).
pub const INACTIVE: Color = Color::Rgb(153, 153, 153);

// ── Semantic ────────────────────────────────────────────────────────────────

pub const SUCCESS: Color = Color::Rgb(78, 186, 101);
pub const ERROR: Color = Color::Rgb(255, 107, 128);
pub const WARNING: Color = Color::Rgb(255, 193, 7);

// ── Accents ─────────────────────────────────────────────────────────────────

/// Bright pink used for bash command borders / indicators.
pub const BASH_BORDER: Color = Color::Rgb(253, 93, 177);

/// Light blue-purple used for suggestions, permissions, and highlights.
pub const SUGGESTION: Color = Color::Rgb(177, 185, 249);

/// Prompt input border color (medium gray).
pub const PROMPT_BORDER: Color = Color::Rgb(136, 136, 136);

/// Plan mode indicator.
pub const PLAN_MODE: Color = Color::Rgb(72, 150, 140);

// ── Backgrounds ─────────────────────────────────────────────────────────────

/// User message background.
pub const USER_MSG_BG: Color = Color::Rgb(55, 55, 55);

/// Bash output / command background.
pub const BASH_MSG_BG: Color = Color::Rgb(65, 60, 65);

// ── Diff ────────────────────────────────────────────────────────────────────

pub const DIFF_ADDED: Color = Color::Rgb(34, 92, 43);
pub const DIFF_REMOVED: Color = Color::Rgb(122, 41, 54);
pub const DIFF_ADDED_WORD: Color = Color::Rgb(56, 166, 96);
pub const DIFF_REMOVED_WORD: Color = Color::Rgb(179, 89, 107);

// ── Unicode figures ─────────────────────────────────────────────────────────

/// Black circle bullet used as message prefix (macOS style).
pub const BLACK_CIRCLE: &str = "⏺";

/// Left-bottom branch used for indented response lines.
pub const RESPONSE_PREFIX: &str = "⎿";

// ── Convenience style builders ──────────────────────────────────────────────

/// Style for the `⏺` message bullet.
pub fn bullet_style() -> Style {
    Style::default().fg(TEXT)
}

/// Style for the `⎿` response indentation prefix (dimmed).
pub fn response_prefix_style() -> Style {
    Style::default().fg(INACTIVE)
}

/// Style for the assistant's body text.
pub fn assistant_text_style() -> Style {
    Style::default().fg(TEXT)
}

/// Style for user message text.
pub fn user_text_style() -> Style {
    Style::default().fg(TEXT)
}

/// Style for tool name (bold).
pub fn tool_name_style() -> Style {
    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
}

/// Style for tool description in parentheses.
pub fn tool_desc_style() -> Style {
    Style::default().fg(INACTIVE)
}

/// Style for bash command text (with pink indicator).
pub fn bash_command_style() -> Style {
    Style::default().fg(TEXT)
}

/// Style for the bash `!` prefix.
pub fn bash_prefix_style() -> Style {
    Style::default().fg(BASH_BORDER)
}

/// Style for success result text.
pub fn success_style() -> Style {
    Style::default().fg(SUCCESS)
}

/// Style for error result text.
pub fn error_style() -> Style {
    Style::default().fg(ERROR)
}

/// Style for warning text.
pub fn warning_style() -> Style {
    Style::default().fg(WARNING)
}

/// Style for the status bar text.
pub fn status_bar_style() -> Style {
    Style::default().fg(INACTIVE)
}

/// Style for the prompt input border.
pub fn prompt_border_style() -> Style {
    Style::default().fg(PROMPT_BORDER)
}

/// Style for disabled / streaming input.
pub fn input_disabled_style() -> Style {
    Style::default().fg(SUBTLE)
}

/// Style for the Claude spinner / thinking indicator.
pub fn spinner_style() -> Style {
    Style::default().fg(CLAUDE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_orange_rgb() {
        assert_eq!(CLAUDE, Color::Rgb(215, 119, 87));
    }

    #[test]
    fn test_text_is_white() {
        assert_eq!(TEXT, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn test_bullet_style_fg() {
        let style = bullet_style();
        assert_eq!(style.fg, Some(TEXT));
    }

    #[test]
    fn test_tool_name_style_is_bold() {
        let style = tool_name_style();
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
