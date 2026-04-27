use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

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
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub provenance: ConfigProvenance,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    Default,
    UserConfig,
    ProjectSettings,
    Env,
    Cli,
}

impl fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            ConfigSource::Default => "default",
            ConfigSource::UserConfig => "user-config",
            ConfigSource::ProjectSettings => "project-settings",
            ConfigSource::Env => "env",
            ConfigSource::Cli => "cli",
        };
        write!(f, "{label}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigProvenance {
    pub model: ConfigSource,
    pub base_url: ConfigSource,
    pub bearer_auth: ConfigSource,
    pub system_prompt: ConfigSource,
    pub max_tokens: ConfigSource,
    pub permission_mode: ConfigSource,
    pub always_allow: ConfigSource,
    pub always_deny: ConfigSource,
    pub stream: ConfigSource,
    pub theme: ConfigSource,
}

impl Default for ConfigProvenance {
    fn default() -> Self {
        Self {
            model: ConfigSource::Default,
            base_url: ConfigSource::Default,
            bearer_auth: ConfigSource::Default,
            system_prompt: ConfigSource::Default,
            max_tokens: ConfigSource::Default,
            permission_mode: ConfigSource::Default,
            always_allow: ConfigSource::Default,
            always_deny: ConfigSource::Default,
            stream: ConfigSource::Default,
            theme: ConfigSource::Default,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let raw: RawConfig = serde_json::from_str(&content)?;

            // Credential resolution: config api_key → ANTHROPIC_API_KEY → ANTHROPIC_AUTH_TOKEN
            let (api_key, bearer_auth) = match raw.api_key {
                Some(api_key) => (api_key, raw.bearer_auth.unwrap_or(false)),
                None => Self::resolve_credential_from_env(raw.bearer_auth)?,
            };

            let mut provenance = ConfigProvenance::default();
            if raw.model.is_some() {
                provenance.model = ConfigSource::UserConfig;
            }
            if raw.base_url.is_some() {
                provenance.base_url = ConfigSource::UserConfig;
            }
            if raw.bearer_auth.is_some() {
                provenance.bearer_auth = ConfigSource::UserConfig;
            }
            if raw.system_prompt.is_some() {
                provenance.system_prompt = ConfigSource::UserConfig;
            }
            if raw.max_tokens.is_some() {
                provenance.max_tokens = ConfigSource::UserConfig;
            }
            if raw.permission_mode.is_some() {
                provenance.permission_mode = ConfigSource::UserConfig;
            }
            if raw.always_allow.is_some() {
                provenance.always_allow = ConfigSource::UserConfig;
            }
            if raw.always_deny.is_some() {
                provenance.always_deny = ConfigSource::UserConfig;
            }
            if raw.stream.is_some() {
                provenance.stream = ConfigSource::UserConfig;
            }
            if raw.theme.is_some() {
                provenance.theme = ConfigSource::UserConfig;
            }

            Ok(Config {
                api_key,
                model: raw.model.unwrap_or_else(default_model),
                base_url: raw.base_url,
                bearer_auth,
                system_prompt: raw.system_prompt,
                max_tokens: raw.max_tokens.unwrap_or_else(default_max_tokens),
                permission_mode: raw.permission_mode.unwrap_or_default(),
                always_allow: raw.always_allow.unwrap_or_default(),
                always_deny: raw.always_deny.unwrap_or_default(),
                stream: raw.stream.unwrap_or_else(default_true),
                theme: raw.theme.unwrap_or_default(),
                provenance,
            })
        } else {
            let (api_key, bearer_auth) = Self::resolve_credential_from_env(None)?;

            Ok(Config {
                api_key,
                model: default_model(),
                base_url: None,
                bearer_auth,
                system_prompt: None,
                max_tokens: default_max_tokens(),
                permission_mode: crate::permission::PermissionMode::Default,
                always_allow: Vec::new(),
                always_deny: Vec::new(),
                stream: true,
                theme: Theme::Dark,
                provenance: ConfigProvenance::default(),
            })
        }
    }

    /// Resolve API credential from environment variables.
    /// Tries ANTHROPIC_API_KEY first (x-api-key), then ANTHROPIC_AUTH_TOKEN (Bearer).
    /// Returns (credential, bearer_auth).
    fn resolve_credential_from_env(
        config_bearer_auth: Option<bool>,
    ) -> Result<(String, bool), ConfigError> {
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            if !api_key.is_empty() {
                return Ok((api_key, config_bearer_auth.unwrap_or(false)));
            }
        }
        if let Ok(auth_token) = std::env::var("ANTHROPIC_AUTH_TOKEN") {
            if !auth_token.is_empty() {
                // ANTHROPIC_AUTH_TOKEN always implies Bearer auth
                return Ok((auth_token, true));
            }
        }
        Err(ConfigError::MissingApiKey)
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

    pub fn save_without_credential(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let raw = RawConfig {
            api_key: None,
            model: Some(self.model.clone()),
            base_url: self.base_url.clone(),
            bearer_auth: Some(self.bearer_auth),
            system_prompt: self.system_prompt.clone(),
            max_tokens: Some(self.max_tokens),
            permission_mode: Some(self.permission_mode),
            always_allow: Some(self.always_allow.clone()),
            always_deny: Some(self.always_deny.clone()),
            stream: Some(self.stream),
            theme: Some(self.theme),
        };
        let content = serde_json::to_string_pretty(&raw)?;
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

    /// Create a Config with just a credential and bearer_auth flag, using defaults for everything else.
    pub fn with_credential(api_key: String, bearer_auth: bool) -> Self {
        Config {
            api_key,
            model: default_model(),
            base_url: None,
            bearer_auth,
            system_prompt: None,
            max_tokens: default_max_tokens(),
            permission_mode: crate::permission::PermissionMode::Default,
            always_allow: Vec::new(),
            always_deny: Vec::new(),
            stream: true,
            theme: Theme::Dark,
            provenance: ConfigProvenance::default(),
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = key.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn default_provenance() -> ConfigProvenance {
        ConfigProvenance::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedField<T> {
    pub value: Option<T>,
    pub source: Option<ConfigSource>,
}

impl<T> ResolvedField<T> {
    pub fn set(&mut self, value: T, source: ConfigSource) {
        self.value = Some(value);
        self.source = Some(source);
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub model: ResolvedField<String>,
    pub base_url: ResolvedField<Option<String>>,
    pub bearer_auth: ResolvedField<bool>,
    pub system_prompt: ResolvedField<Option<String>>,
    pub max_tokens: ResolvedField<u32>,
    pub permission_mode: ResolvedField<crate::permission::PermissionMode>,
    pub always_allow: ResolvedField<Vec<crate::permission::PermissionRule>>,
    pub always_deny: ResolvedField<Vec<crate::permission::PermissionRule>>,
    pub stream: ResolvedField<bool>,
    pub theme: ResolvedField<Theme>,
}

impl Config {
    pub fn apply_overrides(mut self, overrides: ConfigOverrides) -> Self {
        if let Some(value) = overrides.model.value {
            self.model = value;
            self.provenance.model = overrides.model.source.unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.base_url.value {
            self.base_url = value;
            self.provenance.base_url = overrides.base_url.source.unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.bearer_auth.value {
            self.bearer_auth = value;
            self.provenance.bearer_auth = overrides
                .bearer_auth
                .source
                .unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.system_prompt.value {
            self.system_prompt = value;
            self.provenance.system_prompt = overrides
                .system_prompt
                .source
                .unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.max_tokens.value {
            self.max_tokens = value;
            self.provenance.max_tokens =
                overrides.max_tokens.source.unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.permission_mode.value {
            self.permission_mode = value;
            self.provenance.permission_mode = overrides
                .permission_mode
                .source
                .unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.always_allow.value {
            self.always_allow = value;
            self.provenance.always_allow = overrides
                .always_allow
                .source
                .unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.always_deny.value {
            self.always_deny = value;
            self.provenance.always_deny = overrides
                .always_deny
                .source
                .unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.stream.value {
            self.stream = value;
            self.provenance.stream = overrides.stream.source.unwrap_or(ConfigSource::Default);
        }
        if let Some(value) = overrides.theme.value {
            self.theme = value;
            self.provenance.theme = overrides.theme.source.unwrap_or(ConfigSource::Default);
        }
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RawConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bearer_auth: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permission_mode: Option<crate::permission::PermissionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    always_allow: Option<Vec<crate::permission::PermissionRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    always_deny: Option<Vec<crate::permission::PermissionRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<Theme>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("No API credential found. Set ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN")]
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
        old_auth_token: Option<String>,
        temp_home: PathBuf,
    }

    impl TestEnv {
        fn new(name: &str) -> Self {
            let temp_home = make_temp_home(name);
            let old_home = std::env::var("HOME").ok();
            let old_api_key = std::env::var("ANTHROPIC_API_KEY").ok();
            let old_auth_token = std::env::var("ANTHROPIC_AUTH_TOKEN").ok();

            unsafe {
                std::env::set_var("HOME", &temp_home);
                // Clear both credential env vars to isolate tests
                std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
            }

            TestEnv {
                old_home,
                old_api_key,
                old_auth_token,
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

            match &self.old_auth_token {
                Some(value) => unsafe { std::env::set_var("ANTHROPIC_AUTH_TOKEN", value) },
                None => unsafe { std::env::remove_var("ANTHROPIC_AUTH_TOKEN") },
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
            theme: Theme::Light,
            provenance: ConfigProvenance::default(),
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
        assert_eq!(parsed.theme, Theme::Light);
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
        assert_eq!(config.theme, Theme::Dark);
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
        assert_eq!(config.theme, Theme::Dark);
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

    #[test]
    fn test_save_without_credential_omits_api_key() {
        let _guard = env_lock().lock().unwrap();
        let _env = TestEnv::new("save-without-credential");
        let config = Config::with_credential("local-key".to_string(), false)
            .with_model("claude-test");

        config.save_without_credential().unwrap();

        let content = fs::read_to_string(Config::config_path().unwrap()).unwrap();
        assert!(!content.contains("api_key"));
        assert!(content.contains("claude-test"));
    }
}
