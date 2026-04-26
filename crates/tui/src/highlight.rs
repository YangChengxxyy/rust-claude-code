use std::str::FromStr;
use std::sync::LazyLock;

use ratatui::style::{Color as RatatuiColor, Modifier, Style};
use syntect::highlighting::{Color as SyntectColor, Highlighter, Theme, ThemeItem, ThemeSettings};
use syntect::parsing::{ParseState, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::theme::Palette;

// Task 2.1: Lazy-initialized singletons
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<syntect::highlighting::ThemeSet> =
    LazyLock::new(syntect::highlighting::ThemeSet::load_defaults);

/// Access the global default SyntaxSet.
pub fn syntax_set() -> &'static SyntaxSet {
    &SYNTAX_SET
}

/// Access the global default ThemeSet.
pub fn theme_set() -> &'static syntect::highlighting::ThemeSet {
    &THEME_SET
}

// Task 2.2: Custom theme builder
/// Build a custom syntect Theme from a ratatui Palette.
/// Works for both dark and light palettes.
pub fn build_custom_theme(palette: &Palette) -> Theme {
    let mut settings = ThemeSettings::default();

    // Determine if we're in dark mode by checking the text color brightness.
    let is_dark = match palette.text {
        RatatuiColor::Rgb(r, g, b) => {
            let brightness = (r as u32 + g as u32 + b as u32) / 3;
            brightness > 128
        }
        RatatuiColor::White | RatatuiColor::Gray => true,
        _ => false,
    };

    // Background / foreground
    // Note: ratatui controls the actual terminal background, but keep the
    // syntect theme semantically correct so any fallback logic sees sensible
    // contrast values.
    settings.background = Some(ratatui_to_syntect(RatatuiColor::Black));
    settings.foreground = Some(ratatui_to_syntect(palette.text));
    settings.caret = Some(ratatui_to_syntect(palette.text));
    settings.selection = Some(ratatui_to_syntect(palette.subtle));
    settings.line_highlight = Some(ratatui_to_syntect(palette.subtle));

    // Scope-derived colors
    let keyword_color = if is_dark {
        SyntectColor {
            r: 0xCC,
            g: 0x78,
            b: 0x32,
            a: 0xFF,
        }
    } else {
        SyntectColor {
            r: 0xAF,
            g: 0x50,
            b: 0x14,
            a: 0xFF,
        }
    };

    let string_color = ratatui_to_syntect(palette.success);
    let comment_color = ratatui_to_syntect(palette.inactive);

    // Distinct blue/purple for types
    let type_color = if is_dark {
        SyntectColor {
            r: 0x7A,
            g: 0x9E,
            b: 0xC2,
            a: 0xFF,
        }
    } else {
        SyntectColor {
            r: 0x5A,
            g: 0x6E,
            b: 0xA0,
            a: 0xFF,
        }
    };

    let function_color = ratatui_to_syntect(palette.claude);

    // Teal/cyan for numeric constants
    let numeric_color = if is_dark {
        SyntectColor {
            r: 0x2A,
            g: 0xAA,
            b: 0xAA,
            a: 0xFF,
        }
    } else {
        SyntectColor {
            r: 0x1A,
            g: 0x88,
            b: 0x88,
            a: 0xFF,
        }
    };

    let text_color = ratatui_to_syntect(palette.text);

    let mut scopes = vec![];

    // keyword
    scopes.push(theme_item("keyword", keyword_color, None));
    // string
    scopes.push(theme_item("string", string_color, None));
    // comment
    scopes.push(theme_item("comment", comment_color, Some(Modifier::ITALIC)));
    // storage.type, entity.name.type
    scopes.push(theme_item("storage.type", type_color, None));
    scopes.push(theme_item("entity.name.type", type_color, None));
    // entity.name.function
    scopes.push(theme_item("entity.name.function", function_color, None));
    // constant.numeric
    scopes.push(theme_item("constant.numeric", numeric_color, None));
    // keyword.operator, punctuation
    scopes.push(theme_item("keyword.operator", text_color, None));
    scopes.push(theme_item("punctuation", text_color, None));
    // variable
    scopes.push(theme_item("variable", text_color, None));

    Theme {
        name: Some("custom".to_string()),
        author: Some("rust-claude-tui".to_string()),
        settings,
        scopes,
    }
}

fn theme_item(scope: &str, color: SyntectColor, modifier: Option<Modifier>) -> ThemeItem {
    let mut style = syntect::highlighting::StyleModifier::default();
    style.foreground = Some(color);
    if let Some(m) = modifier {
        let mut fs = syntect::highlighting::FontStyle::default();
        if m.contains(Modifier::BOLD) {
            fs = fs | syntect::highlighting::FontStyle::BOLD;
        }
        if m.contains(Modifier::ITALIC) {
            fs = fs | syntect::highlighting::FontStyle::ITALIC;
        }
        if m.contains(Modifier::UNDERLINED) {
            fs = fs | syntect::highlighting::FontStyle::UNDERLINE;
        }
        style.font_style = Some(fs);
    }
    ThemeItem {
        scope: syntect::highlighting::ScopeSelectors::from_str(scope).unwrap(),
        style,
    }
}

fn ratatui_to_syntect(color: RatatuiColor) -> SyntectColor {
    match color {
        RatatuiColor::Rgb(r, g, b) => SyntectColor { r, g, b, a: 0xFF },
        RatatuiColor::Black => SyntectColor {
            r: 0x00,
            g: 0x00,
            b: 0x00,
            a: 0xFF,
        },
        RatatuiColor::Red => SyntectColor {
            r: 0xFF,
            g: 0x00,
            b: 0x00,
            a: 0xFF,
        },
        RatatuiColor::Green => SyntectColor {
            r: 0x00,
            g: 0xFF,
            b: 0x00,
            a: 0xFF,
        },
        RatatuiColor::Yellow => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0x00,
            a: 0xFF,
        },
        RatatuiColor::Blue => SyntectColor {
            r: 0x00,
            g: 0x00,
            b: 0xFF,
            a: 0xFF,
        },
        RatatuiColor::Magenta => SyntectColor {
            r: 0xFF,
            g: 0x00,
            b: 0xFF,
            a: 0xFF,
        },
        RatatuiColor::Cyan => SyntectColor {
            r: 0x00,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
        RatatuiColor::Gray => SyntectColor {
            r: 0x80,
            g: 0x80,
            b: 0x80,
            a: 0xFF,
        },
        RatatuiColor::DarkGray => SyntectColor {
            r: 0x40,
            g: 0x40,
            b: 0x40,
            a: 0xFF,
        },
        RatatuiColor::LightRed => SyntectColor {
            r: 0xFF,
            g: 0x80,
            b: 0x80,
            a: 0xFF,
        },
        RatatuiColor::LightGreen => SyntectColor {
            r: 0x80,
            g: 0xFF,
            b: 0x80,
            a: 0xFF,
        },
        RatatuiColor::LightYellow => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0x80,
            a: 0xFF,
        },
        RatatuiColor::LightBlue => SyntectColor {
            r: 0x80,
            g: 0x80,
            b: 0xFF,
            a: 0xFF,
        },
        RatatuiColor::LightMagenta => SyntectColor {
            r: 0xFF,
            g: 0x80,
            b: 0xFF,
            a: 0xFF,
        },
        RatatuiColor::LightCyan => SyntectColor {
            r: 0x80,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
        RatatuiColor::White => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
        _ => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
    }
}

fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let mut ratatui_style = Style::default().fg(syntect_to_ratatui(style.foreground));
    if style
        .font_style
        .contains(syntect::highlighting::FontStyle::BOLD)
    {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if style
        .font_style
        .contains(syntect::highlighting::FontStyle::ITALIC)
    {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if style
        .font_style
        .contains(syntect::highlighting::FontStyle::UNDERLINE)
    {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }
    ratatui_style
}

fn syntect_to_ratatui(color: SyntectColor) -> RatatuiColor {
    RatatuiColor::Rgb(color.r, color.g, color.b)
}

// Task 2.3: Line highlighting
/// Highlight a single line and return ratatui-compatible styled spans.
///
/// `parse_state` tracks the parsing context across lines (scope stack for
/// determining which syntax rules apply). `highlight_state` tracks the
/// highlighting context across lines (accumulated scopes for correct
/// coloring of multi-line constructs like block comments and strings).
/// Both must be maintained across calls for correct incremental highlighting.
pub fn highlight_line(
    line: &str,
    parse_state: &mut ParseState,
    highlight_state: &mut syntect::highlighting::HighlightState,
    highlighter: &Highlighter,
) -> Vec<(Style, String)> {
    let ops = parse_state
        .parse_line(line, &SYNTAX_SET)
        .unwrap_or_default();
    let iter =
        syntect::highlighting::HighlightIterator::new(highlight_state, &ops, line, highlighter);
    iter.map(|(style, text)| (syntect_style_to_ratatui(style), text.to_string()))
        .collect()
}

/// Create a new HighlightState for the start of a code block.
pub fn new_highlight_state(highlighter: &Highlighter) -> syntect::highlighting::HighlightState {
    syntect::highlighting::HighlightState::new(highlighter, syntect::parsing::ScopeStack::new())
}

// Task 2.4: Language resolution
/// Resolve a language identifier to a SyntaxReference.
/// Supports common aliases and falls back through multiple lookup strategies.
/// For languages not in syntect defaults (e.g., TypeScript, TOML), maps to the
/// closest available syntax (TypeScript→JavaScript).
pub fn resolve_syntax<'a>(
    language: &str,
    syntax_set: &'a SyntaxSet,
) -> Option<&'a SyntaxReference> {
    let normalized = language.to_lowercase();

    // Map aliases to file-extension tokens that syntect knows about.
    // syntect's default set uses file extensions for token lookup.
    let token = match normalized.as_str() {
        "ts" | "tsx" | "typescript" => "js", // TypeScript not in defaults, use JS
        "py" | "python" => "py",
        "sh" | "shell" => "sh",
        "bash" | "zsh" => "bash",
        "js" | "jsx" | "javascript" => "js",
        "yml" => "yaml",
        "toml" => "toml", // Not in defaults — will gracefully fail
        "c++" | "cpp" | "cxx" => "cpp",
        "c#" | "csharp" => "cs",
        other => other,
    };

    // First try by file-extension token (most reliable for syntect defaults)
    syntax_set
        .find_syntax_by_token(token)
        // Then try by exact name
        .or_else(|| syntax_set.find_syntax_by_name(language))
        // Then try by name with original input
        .or_else(|| syntax_set.find_syntax_by_name(&normalized))
        // Then try by extension with original input
        .or_else(|| syntax_set.find_syntax_by_extension(&normalized))
}

// Task 2.5: Full code block API
/// Highlight a complete code block, returning styled spans per line.
/// If language is None or unrecognized, returns each line as a single span
/// with default text color.
pub fn highlight_code_block(
    code: &str,
    language: Option<&str>,
    palette: &Palette,
) -> Vec<Vec<(Style, String)>> {
    let syntax_set = syntax_set();

    let syntax = match language {
        Some(lang) => resolve_syntax(lang, syntax_set),
        None => None,
    };

    match syntax {
        Some(syntax) => {
            let theme = build_custom_theme(palette);
            let highlighter = Highlighter::new(&theme);
            let mut parse_state = ParseState::new(syntax);
            let mut hl_state = new_highlight_state(&highlighter);

            let mut result = Vec::new();
            for line in LinesWithEndings::from(code) {
                let spans = highlight_line(line, &mut parse_state, &mut hl_state, &highlighter);
                result.push(spans);
            }
            result
        }
        None => {
            // Fallback: each line as a single span with default text color
            code.lines()
                .map(|line| vec![(Style::default().fg(palette.text), line.to_string())])
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_code_produces_distinct_colors() {
        let palette = Palette::dark();
        let code = "fn main() {}";
        let lines = highlight_code_block(code, Some("rust"), &palette);
        assert_eq!(lines.len(), 1);
        let spans = &lines[0];
        // Should produce multiple spans
        assert!(
            spans.len() > 1,
            "Expected multiple spans for Rust code, got {}",
            spans.len()
        );

        // Collect distinct foreground colors
        let colors: std::collections::HashSet<_> = spans.iter().filter_map(|(s, _)| s.fg).collect();
        assert!(
            colors.len() >= 2,
            "Expected at least 2 distinct colors, got {:?}",
            colors
        );
    }

    #[test]
    fn test_python_code_has_keyword_highlighting() {
        let palette = Palette::dark();
        let code = "def foo(): pass";
        let lines = highlight_code_block(code, Some("python"), &palette);
        assert_eq!(lines.len(), 1);
        let spans = &lines[0];
        assert!(
            spans.len() > 1,
            "Expected multiple spans for Python code, got {}",
            spans.len()
        );

        // Check that "def" or "pass" is highlighted with a non-default color
        let has_keyword_color = spans.iter().any(|(style, text)| {
            let is_keyword = text.trim() == "def" || text.trim() == "pass";
            let non_default = style.fg != Some(palette.text);
            is_keyword && non_default
        });
        assert!(
            has_keyword_color,
            "Expected keyword highlighting in Python code"
        );
    }

    #[test]
    fn test_unsupported_language_fallback() {
        let palette = Palette::dark();
        let code = "++++[>++++<-]";
        let lines = highlight_code_block(code, Some("brainfuck"), &palette);
        assert_eq!(lines.len(), 1);
        let spans = &lines[0];
        assert_eq!(
            spans.len(),
            1,
            "Expected single span fallback for unsupported language"
        );
        assert_eq!(spans[0].0.fg, Some(palette.text));
        assert_eq!(spans[0].1, code);
    }

    #[test]
    fn test_no_language_fallback() {
        let palette = Palette::dark();
        let code = "hello world";
        let lines = highlight_code_block(code, None, &palette);
        assert_eq!(lines.len(), 1);
        let spans = &lines[0];
        assert_eq!(
            spans.len(),
            1,
            "Expected single span fallback when no language provided"
        );
        assert_eq!(spans[0].0.fg, Some(palette.text));
        assert_eq!(spans[0].1, code);
    }

    #[test]
    fn test_alias_ts_to_javascript() {
        // TypeScript not in syntect defaults, falls back to JavaScript
        let ss = syntax_set();
        let syntax = resolve_syntax("ts", ss);
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "JavaScript");
    }

    #[test]
    fn test_alias_py_to_python() {
        let ss = syntax_set();
        let syntax = resolve_syntax("py", ss);
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Python");
    }

    #[test]
    fn test_alias_sh_to_shell() {
        let ss = syntax_set();
        let syntax = resolve_syntax("sh", ss);
        assert!(syntax.is_some());
        let name = syntax.unwrap().name.as_str();
        assert!(
            name.contains("Shell")
                || name.contains("Bourne")
                || name.contains("Bash")
                || name.contains("bash"),
            "Expected Shell/Bash syntax, got {}",
            name
        );
    }

    #[test]
    fn test_alias_bash_to_shell() {
        let ss = syntax_set();
        let syntax = resolve_syntax("bash", ss);
        assert!(syntax.is_some());
        let name = syntax.unwrap().name.as_str();
        assert!(
            name.contains("Shell")
                || name.contains("Bourne")
                || name.contains("Bash")
                || name.contains("bash"),
            "Expected Shell/Bash syntax, got {}",
            name
        );
    }

    #[test]
    fn test_alias_js_to_javascript() {
        let ss = syntax_set();
        let syntax = resolve_syntax("js", ss);
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "JavaScript");
    }

    #[test]
    fn test_alias_yml_to_yaml() {
        let ss = syntax_set();
        let syntax = resolve_syntax("yml", ss);
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "YAML");
    }

    #[test]
    fn test_alias_tsx_to_javascript() {
        // TypeScript not in syntect defaults, falls back to JavaScript
        let ss = syntax_set();
        let syntax = resolve_syntax("tsx", ss);
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "JavaScript");
    }

    #[test]
    fn test_alias_jsx_to_javascript() {
        let ss = syntax_set();
        let syntax = resolve_syntax("jsx", ss);
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "JavaScript");
    }

    #[test]
    fn test_alias_toml_graceful_fallback() {
        // TOML not in syntect defaults — should return None (graceful fallback)
        let ss = syntax_set();
        let syntax = resolve_syntax("toml", ss);
        // TOML may or may not exist depending on syntect version;
        // if not found, highlight_code_block will fall back to plain text
        if syntax.is_some() {
            assert!(syntax.unwrap().name.contains("TOML"));
        }
        // No assertion failure — either found or graceful None
    }
}
