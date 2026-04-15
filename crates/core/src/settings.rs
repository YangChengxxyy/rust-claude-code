use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Representation of `~/.claude/settings.json`.
/// Reads `env`, `model`, and `apiKeyHelper` fields.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Top-level model override from settings (e.g. "opus[1m]").
    #[serde(default)]
    pub model: Option<String>,

    /// Script/command that outputs an API credential to stdout.
    #[serde(default, rename = "apiKeyHelper")]
    pub api_key_helper: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("HOME environment variable not set")]
    NoHomeDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl ClaudeSettings {
    /// Load settings from `~/.claude/settings.json`.
    /// Returns an empty settings object if the file does not exist.
    pub fn load() -> Result<Self, SettingsError> {
        let path = Self::default_path()?;
        Self::load_from(&path)
    }

    /// Load settings from a custom path.
    /// Returns an empty settings object if the file does not exist.
    pub fn load_from(path: &std::path::Path) -> Result<Self, SettingsError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let settings: ClaudeSettings = serde_json::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(ClaudeSettings::default())
        }
    }

    /// The default path: `$CLAUDE_CONFIG_DIR/settings.json` or `$HOME/.claude/settings.json`
    fn default_path() -> Result<PathBuf, SettingsError> {
        if let Ok(config_dir) = std::env::var("CLAUDE_CONFIG_DIR") {
            return Ok(PathBuf::from(config_dir).join("settings.json"));
        }
        let home = std::env::var("HOME").map_err(|_| SettingsError::NoHomeDir)?;
        Ok(PathBuf::from(home).join(".claude").join("settings.json"))
    }

    /// Apply env vars from this settings object into the process environment.
    /// Does NOT overwrite existing env vars — shell environment has higher priority.
    pub fn apply_env(&self) {
        for (key, value) in &self.env {
            if std::env::var(key).is_err() {
                unsafe {
                    std::env::set_var(key, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!("rust-claude-settings-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let dir = make_temp_dir("missing");
        let path = dir.join("nonexistent.json");
        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert!(settings.env.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_valid_file() {
        let dir = make_temp_dir("valid");
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{"env":{"ANTHROPIC_MODEL":"claude-opus-4-20250514","ANTHROPIC_BASE_URL":"http://localhost:8080"}}"#,
        )
        .unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert_eq!(
            settings.env.get("ANTHROPIC_MODEL").unwrap(),
            "claude-opus-4-20250514"
        );
        assert_eq!(
            settings.env.get("ANTHROPIC_BASE_URL").unwrap(),
            "http://localhost:8080"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_file_without_env_field() {
        let dir = make_temp_dir("no-env");
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"model":"claude-sonnet-4-20250514"}"#).unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert!(settings.env.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_apply_env_sets_missing_keys() {
        let mut env = HashMap::new();
        let test_key = "RUST_CLAUDE_TEST_SETTINGS_APPLY_KEY";
        // Ensure key is not set
        std::env::remove_var(test_key);
        env.insert(
            test_key.to_string(),
            "from-settings".to_string(),
        );

        let settings = ClaudeSettings { env, ..Default::default() };
        settings.apply_env();

        assert_eq!(std::env::var(test_key).unwrap(), "from-settings");
        std::env::remove_var(test_key);
    }

    #[test]
    fn test_apply_env_does_not_overwrite_existing() {
        let test_key = "RUST_CLAUDE_TEST_SETTINGS_NO_OVERWRITE";
        std::env::set_var(test_key, "from-shell");

        let mut env = HashMap::new();
        env.insert(test_key.to_string(), "from-settings".to_string());

        let settings = ClaudeSettings { env, ..Default::default() };
        settings.apply_env();

        assert_eq!(std::env::var(test_key).unwrap(), "from-shell");
        std::env::remove_var(test_key);
    }
}
