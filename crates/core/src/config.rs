use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub bearer_auth: bool,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub permission_mode: crate::permission::PermissionMode,
    #[serde(default)]
    pub always_allow: Vec<crate::permission::PermissionRule>,
    #[serde(default)]
    pub always_deny: Vec<crate::permission::PermissionRule>,
    #[serde(default = "default_true")]
    pub stream: bool,
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_max_tokens() -> u32 {
    16384
}

fn default_true() -> bool {
    true
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let raw: RawConfig = serde_json::from_str(&content)?;
            let api_key = match raw.api_key {
                Some(api_key) => api_key,
                None => {
                    std::env::var("ANTHROPIC_API_KEY").map_err(|_| ConfigError::MissingApiKey)?
                }
            };

            Ok(Config {
                api_key,
                model: raw.model.unwrap_or_else(default_model),
                base_url: raw.base_url,
                bearer_auth: raw.bearer_auth.unwrap_or(false),
                system_prompt: raw.system_prompt,
                max_tokens: raw.max_tokens.unwrap_or_else(default_max_tokens),
                permission_mode: raw.permission_mode.unwrap_or_default(),
                always_allow: raw.always_allow.unwrap_or_default(),
                always_deny: raw.always_deny.unwrap_or_default(),
                stream: raw.stream.unwrap_or_else(default_true),
            })
        } else {
            let api_key =
                std::env::var("ANTHROPIC_API_KEY").map_err(|_| ConfigError::MissingApiKey)?;

            Ok(Config {
                api_key,
                model: default_model(),
                base_url: None,
                bearer_auth: false,
                system_prompt: None,
                max_tokens: default_max_tokens(),
                permission_mode: crate::permission::PermissionMode::Default,
                always_allow: Vec::new(),
                always_deny: Vec::new(),
                stream: true,
            })
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf, ConfigError> {
        let home = std::env::var("HOME").map_err(|_| ConfigError::NoHomeDir)?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("rust-claude-code")
            .join("config.json"))
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = key.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawConfig {
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    bearer_auth: Option<bool>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    permission_mode: Option<crate::permission::PermissionMode>,
    #[serde(default)]
    always_allow: Option<Vec<crate::permission::PermissionRule>>,
    #[serde(default)]
    always_deny: Option<Vec<crate::permission::PermissionRule>>,
    #[serde(default)]
    stream: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("ANTHROPIC_API_KEY environment variable not set")]
    MissingApiKey,
    #[error("HOME environment variable not set")]
    NoHomeDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn make_temp_home(name: &str) -> PathBuf {
        let unique = format!("rust-claude-code-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    struct TestEnv {
        old_home: Option<String>,
        old_api_key: Option<String>,
        temp_home: PathBuf,
    }

    impl TestEnv {
        fn new(name: &str) -> Self {
            let temp_home = make_temp_home(name);
            let old_home = std::env::var("HOME").ok();
            let old_api_key = std::env::var("ANTHROPIC_API_KEY").ok();

            unsafe {
                std::env::set_var("HOME", &temp_home);
            }

            TestEnv {
                old_home,
                old_api_key,
                temp_home,
            }
        }

        fn set_api_key(&self, value: &str) {
            unsafe {
                std::env::set_var("ANTHROPIC_API_KEY", value);
            }
        }

        fn remove_api_key(&self) {
            unsafe {
                std::env::remove_var("ANTHROPIC_API_KEY");
            }
        }

        fn config_path(&self) -> PathBuf {
            self.temp_home
                .join(".config")
                .join("rust-claude-code")
                .join("config.json")
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            match &self.old_home {
                Some(value) => unsafe { std::env::set_var("HOME", value) },
                None => unsafe { std::env::remove_var("HOME") },
            }

            match &self.old_api_key {
                Some(value) => unsafe { std::env::set_var("ANTHROPIC_API_KEY", value) },
                None => unsafe { std::env::remove_var("ANTHROPIC_API_KEY") },
            }

            let _ = fs::remove_dir_all(&self.temp_home);
        }
    }

    #[test]
    fn test_config_builder() {
        let config = Config {
            api_key: "test-key".to_string(),
            model: "claude-3-opus".to_string(),
            base_url: None,
            bearer_auth: false,
            system_prompt: Some("You are a test assistant".to_string()),
            max_tokens: 4096,
            permission_mode: crate::permission::PermissionMode::BypassPermissions,
            always_allow: vec![],
            always_deny: vec![],
            stream: false,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.api_key, "test-key");
        assert_eq!(parsed.model, "claude-3-opus");
        assert_eq!(
            parsed.system_prompt.as_deref(),
            Some("You are a test assistant")
        );
        assert_eq!(parsed.max_tokens, 4096);
        assert!(!parsed.stream);
    }

    #[test]
    fn test_config_defaults() {
        let json = r#"{"api_key":"key123"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_key, "key123");
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert!(config.system_prompt.is_none());
        assert_eq!(config.max_tokens, 16384);
        assert!(config.stream);
    }

    #[test]
    fn test_load_uses_env_api_key_when_config_file_omits_it() {
        let _guard = env_lock().lock().unwrap();
        let env = TestEnv::new("config-env-fallback");
        env.set_api_key("env-key");

        let config_path = env.config_path();
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            r#"{
  "model": "claude-test",
  "max_tokens": 2048,
  "stream": false
}"#,
        )
        .unwrap();

        let config = Config::load().unwrap();
        assert_eq!(config.api_key, "env-key");
        assert_eq!(config.model, "claude-test");
        assert_eq!(config.max_tokens, 2048);
        assert!(!config.stream);
    }

    #[test]
    fn test_load_prefers_config_api_key_over_env() {
        let _guard = env_lock().lock().unwrap();
        let env = TestEnv::new("config-preferred-key");
        env.set_api_key("env-key");

        let config_path = env.config_path();
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            r#"{
  "api_key": "file-key"
}"#,
        )
        .unwrap();

        let config = Config::load().unwrap();
        assert_eq!(config.api_key, "file-key");
    }

    #[test]
    fn test_load_errors_when_no_api_key_available() {
        let _guard = env_lock().lock().unwrap();
        let env = TestEnv::new("config-missing-key");
        env.remove_api_key();

        let config_path = env.config_path();
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, r#"{"model":"claude-test"}"#).unwrap();

        let err = Config::load().unwrap_err();
        assert!(matches!(err, ConfigError::MissingApiKey));
    }
}
