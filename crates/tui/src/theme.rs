//! Centralized color theme support for the TUI.

use ratatui::style::{Color, Modifier, Style};
use rust_claude_core::config::Theme as ConfigTheme;
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Deserialize)]
struct CustomThemeFile {
    claude: String,
    text: String,
    subtle: String,
    inactive: String,
    success: String,
    error: String,
    warning: String,
    bash_border: String,
    suggestion: String,
    prompt_border: String,
    plan_mode: String,
    user_msg_bg: String,
    bash_msg_bg: String,
    diff_added: String,
    diff_removed: String,
    diff_added_word: String,
    diff_removed_word: String,
}

pub fn custom_theme_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("rust-claude-code")
        .join("theme.json")
}

pub fn load_custom_palette_default() -> Result<Palette, String> {
    load_custom_palette(&custom_theme_path())
}

pub fn load_custom_palette(path: &Path) -> Result<Palette, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let theme: CustomThemeFile = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
    Ok(Palette {
        claude: parse_rgb_color("claude", &theme.claude)?,
        text: parse_rgb_color("text", &theme.text)?,
        subtle: parse_rgb_color("subtle", &theme.subtle)?,
        inactive: parse_rgb_color("inactive", &theme.inactive)?,
        success: parse_rgb_color("success", &theme.success)?,
        error: parse_rgb_color("error", &theme.error)?,
        warning: parse_rgb_color("warning", &theme.warning)?,
        bash_border: parse_rgb_color("bash_border", &theme.bash_border)?,
        suggestion: parse_rgb_color("suggestion", &theme.suggestion)?,
        prompt_border: parse_rgb_color("prompt_border", &theme.prompt_border)?,
        plan_mode: parse_rgb_color("plan_mode", &theme.plan_mode)?,
        user_msg_bg: parse_rgb_color("user_msg_bg", &theme.user_msg_bg)?,
        bash_msg_bg: parse_rgb_color("bash_msg_bg", &theme.bash_msg_bg)?,
        diff_added: parse_rgb_color("diff_added", &theme.diff_added)?,
        diff_removed: parse_rgb_color("diff_removed", &theme.diff_removed)?,
        diff_added_word: parse_rgb_color("diff_added_word", &theme.diff_added_word)?,
        diff_removed_word: parse_rgb_color("diff_removed_word", &theme.diff_removed_word)?,
    })
}

fn parse_rgb_color(field: &str, value: &str) -> Result<Color, String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a #RRGGBB color"));
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| format!("{field} has an invalid red channel"))?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| format!("{field} has an invalid green channel"))?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| format!("{field} has an invalid blue channel"))?;
    Ok(Color::Rgb(r, g, b))
}

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

    #[test]
    fn test_load_custom_palette() {
        let path = std::env::temp_dir().join(format!(
            "theme-test-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("thread")
        ));
        std::fs::write(
            &path,
            r##"{
  "claude": "#010203",
  "text": "#111111",
  "subtle": "#222222",
  "inactive": "#333333",
  "success": "#444444",
  "error": "#555555",
  "warning": "#666666",
  "bash_border": "#777777",
  "suggestion": "#888888",
  "prompt_border": "#999999",
  "plan_mode": "#aaaaaa",
  "user_msg_bg": "#bbbbbb",
  "bash_msg_bg": "#cccccc",
  "diff_added": "#dddddd",
  "diff_removed": "#eeeeee",
  "diff_added_word": "#123456",
  "diff_removed_word": "#abcdef"
}"##,
        )
        .unwrap();

        let palette = load_custom_palette(&path).unwrap();
        assert_eq!(palette.claude, Color::Rgb(1, 2, 3));
        assert_eq!(palette.diff_removed_word, Color::Rgb(171, 205, 239));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_custom_palette_rejects_invalid_color() {
        let path = std::env::temp_dir().join(format!(
            "theme-invalid-test-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("thread")
        ));
        std::fs::write(
            &path,
            r##"{
  "claude": "nope",
  "text": "#111111",
  "subtle": "#222222",
  "inactive": "#333333",
  "success": "#444444",
  "error": "#555555",
  "warning": "#666666",
  "bash_border": "#777777",
  "suggestion": "#888888",
  "prompt_border": "#999999",
  "plan_mode": "#aaaaaa",
  "user_msg_bg": "#bbbbbb",
  "bash_msg_bg": "#cccccc",
  "diff_added": "#dddddd",
  "diff_removed": "#eeeeee",
  "diff_added_word": "#123456",
  "diff_removed_word": "#abcdef"
}"##,
        )
        .unwrap();

        let error = load_custom_palette(&path).unwrap_err();
        assert!(error.contains("claude must be a #RRGGBB color"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_custom_palette_reports_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "missing-theme-test-{}-{}.json",
            std::process::id(),
            chrono_like_unique_suffix()
        ));
        let error = load_custom_palette(&path).unwrap_err();
        assert!(error.contains("failed to read"));
    }

    #[test]
    fn test_load_custom_palette_reports_invalid_json() {
        let path = std::env::temp_dir().join(format!(
            "invalid-json-theme-test-{}-{}.json",
            std::process::id(),
            chrono_like_unique_suffix()
        ));
        std::fs::write(&path, "{not json").unwrap();

        let error = load_custom_palette(&path).unwrap_err();
        assert!(error.contains("failed to parse"));
        let _ = std::fs::remove_file(path);
    }

    fn chrono_like_unique_suffix() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos().to_string())
            .unwrap_or_else(|_| "0".into())
    }
}
