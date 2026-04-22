use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LspLanguage {
    Rust,
    TypeScript,
    Python,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCommand {
    pub command: String,
    pub args: Vec<String>,
}

pub fn detect_language_from_path(path: &Path) -> Option<LspLanguage> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some(LspLanguage::Rust),
        Some("ts") | Some("tsx") | Some("js") | Some("jsx") => Some(LspLanguage::TypeScript),
        Some("py") => Some(LspLanguage::Python),
        _ => None,
    }
}

pub fn discover_server_command(language: LspLanguage) -> ServerCommand {
    match language {
        LspLanguage::Rust => ServerCommand {
            command: "rust-analyzer".to_string(),
            args: vec![],
        },
        LspLanguage::TypeScript => ServerCommand {
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
        },
        LspLanguage::Python => ServerCommand {
            command: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_languages() {
        assert_eq!(detect_language_from_path(Path::new("main.rs")), Some(LspLanguage::Rust));
        assert_eq!(detect_language_from_path(Path::new("index.ts")), Some(LspLanguage::TypeScript));
        assert_eq!(detect_language_from_path(Path::new("app.py")), Some(LspLanguage::Python));
        assert_eq!(detect_language_from_path(Path::new("README.md")), None);
    }

    #[test]
    fn discovers_server_commands() {
        assert_eq!(discover_server_command(LspLanguage::Rust).command, "rust-analyzer");
        assert_eq!(discover_server_command(LspLanguage::TypeScript).args, vec!["--stdio"]);
        assert_eq!(discover_server_command(LspLanguage::Python).args, vec!["--stdio"]);
    }
}
