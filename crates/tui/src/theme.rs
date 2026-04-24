//! Centralized color theme support for the TUI.

use ratatui::style::{Color, Modifier, Style};
use rust_claude_core::config::Theme as ConfigTheme;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub claude: Color,
    pub text: Color,
    pub subtle: Color,
    pub inactive: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub bash_border: Color,
    pub suggestion: Color,
    pub prompt_border: Color,
    pub plan_mode: Color,
    pub user_msg_bg: Color,
    pub bash_msg_bg: Color,
    pub diff_added: Color,
    pub diff_removed: Color,
    pub diff_added_word: Color,
    pub diff_removed_word: Color,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            claude: Color::Rgb(215, 119, 87),
            text: Color::Rgb(255, 255, 255),
            subtle: Color::Rgb(80, 80, 80),
            inactive: Color::Rgb(153, 153, 153),
            success: Color::Rgb(78, 186, 101),
            error: Color::Rgb(255, 107, 128),
            warning: Color::Rgb(255, 193, 7),
            bash_border: Color::Rgb(253, 93, 177),
            suggestion: Color::Rgb(177, 185, 249),
            prompt_border: Color::Rgb(136, 136, 136),
            plan_mode: Color::Rgb(72, 150, 140),
            user_msg_bg: Color::Rgb(55, 55, 55),
            bash_msg_bg: Color::Rgb(65, 60, 65),
            diff_added: Color::Rgb(34, 92, 43),
            diff_removed: Color::Rgb(122, 41, 54),
            diff_added_word: Color::Rgb(56, 166, 96),
            diff_removed_word: Color::Rgb(179, 89, 107),
        }
    }

    pub fn light() -> Self {
        Self {
            claude: Color::Rgb(180, 90, 55),
            text: Color::Rgb(20, 20, 20),
            subtle: Color::Rgb(180, 180, 180),
            inactive: Color::Rgb(110, 110, 110),
            success: Color::Rgb(34, 139, 34),
            error: Color::Rgb(196, 48, 48),
            warning: Color::Rgb(180, 120, 0),
            bash_border: Color::Rgb(180, 70, 140),
            suggestion: Color::Rgb(85, 110, 220),
            prompt_border: Color::Rgb(150, 150, 150),
            plan_mode: Color::Rgb(60, 130, 120),
            user_msg_bg: Color::Rgb(235, 235, 235),
            bash_msg_bg: Color::Rgb(240, 235, 240),
            diff_added: Color::Rgb(210, 245, 210),
            diff_removed: Color::Rgb(250, 220, 225),
            diff_added_word: Color::Rgb(60, 150, 60),
            diff_removed_word: Color::Rgb(200, 90, 90),
        }
    }

    pub fn from_config(theme: ConfigTheme) -> Self {
        match theme {
            ConfigTheme::Dark => Self::dark(),
            ConfigTheme::Light => Self::light(),
        }
    }

    pub fn bullet_style(self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn response_prefix_style(self) -> Style {
        Style::default().fg(self.inactive)
    }

    pub fn assistant_text_style(self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn user_text_style(self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn tool_name_style(self) -> Style {
        Style::default().fg(self.text).add_modifier(Modifier::BOLD)
    }

    pub fn tool_desc_style(self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn bash_command_style(self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn bash_prefix_style(self) -> Style {
        Style::default().fg(self.bash_border)
    }

    pub fn success_style(self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn error_style(self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn warning_style(self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn status_bar_style(self) -> Style {
        Style::default().fg(self.inactive)
    }

    pub fn prompt_border_style(self) -> Style {
        Style::default().fg(self.prompt_border)
    }

    pub fn input_disabled_style(self) -> Style {
        Style::default().fg(self.subtle)
    }

    pub fn spinner_style(self) -> Style {
        Style::default().fg(self.claude)
    }
}

pub const BLACK_CIRCLE: &str = "⏺";
pub const ASSISTANT_BULLET: &str = "•";
pub const RESPONSE_PREFIX: &str = "⎿";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_text_is_white() {
        assert_eq!(Palette::dark().text, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn test_light_text_is_dark() {
        assert_eq!(Palette::light().text, Color::Rgb(20, 20, 20));
    }

    #[test]
    fn test_assistant_bullet_is_small_dot() {
        assert_eq!(ASSISTANT_BULLET, "•");
    }

    #[test]
    fn test_bullet_style_fg() {
        let style = Palette::dark().bullet_style();
        assert_eq!(style.fg, Some(Palette::dark().text));
    }

    #[test]
    fn test_tool_desc_style_is_bright() {
        let style = Palette::dark().tool_desc_style();
        assert_eq!(style.fg, Some(Palette::dark().text));
    }

    #[test]
    fn test_tool_name_style_is_bold() {
        let style = Palette::dark().tool_name_style();
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
