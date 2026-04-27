use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::hooks::HooksConfig;
use crate::mcp_config::{merge_mcp_servers, McpServersConfig};

#[cfg(test)]
use crate::hooks::HookEventGroup;
use crate::permission::{PermissionError, PermissionRule, RuleType};

/// Representation of `~/.claude/settings.json` or project `.claude/settings.json`.
/// Reads `env`, `model`, `apiKeyHelper`, and `permissions` fields.
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

    #[serde(default)]
    pub permissions: SettingsPermissions,

    /// Hook definitions keyed by event name (e.g. "PreToolUse").
    #[serde(default)]
    pub hooks: HooksConfig,

    /// MCP server definitions keyed by server name.
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: McpServersConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsPermissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedPermissions {
    pub allow: Vec<PermissionRule>,
    pub deny: Vec<PermissionRule>,
}

#[derive(Debug, Clone)]
pub struct SettingsLayer {
    pub path: PathBuf,
    pub settings: ClaudeSettings,
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("HOME environment variable not set")]
    NoHomeDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("permission parse error: {0}")]
    Permission(#[from] PermissionError),
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
    pub fn load_from(path: &Path) -> Result<Self, SettingsError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let settings: ClaudeSettings = serde_json::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(ClaudeSettings::default())
        }
    }

    pub fn load_layer_from(path: &Path) -> Result<Option<SettingsLayer>, SettingsError> {
        if !path.exists() {
            return Ok(None);
        }
        let settings = Self::load_from(path)?;
        Ok(Some(SettingsLayer {
            path: path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
            settings,
        }))
    }

    pub fn discover_project_settings(cwd: &Path) -> Option<PathBuf> {
        crate::claude_md::project_discovery_dirs(cwd)
            .into_iter()
            .find_map(|dir| {
                let path = dir.join(".claude").join("settings.json");
                path.exists().then_some(path)
            })
    }

    /// Discover the local project settings file (`.claude/settings.local.json`).
    /// Looks in the same directory as `.claude/settings.json`.
    pub fn discover_project_local_settings(cwd: &Path) -> Option<PathBuf> {
        crate::claude_md::project_discovery_dirs(cwd)
            .into_iter()
            .find_map(|dir| {
                let path = dir.join(".claude").join("settings.local.json");
                path.exists().then_some(path)
            })
    }

    /// Load project settings, merging shared and local layers.
    /// Returns a single merged layer with the local-project settings taking
    /// priority over shared-project settings.
    pub fn load_project(cwd: &Path) -> Result<Option<SettingsLayer>, SettingsError> {
        let shared = match Self::discover_project_settings(cwd) {
            Some(path) => Self::load_layer_from(&path)?,
            None => None,
        };
        let local = match Self::discover_project_local_settings(cwd) {
            Some(path) => Self::load_layer_from(&path)?,
            None => None,
        };

        match (shared, local) {
            (None, None) => Ok(None),
            (Some(s), None) => Ok(Some(s)),
            (None, Some(l)) => Ok(Some(l)),
            (Some(s), Some(l)) => {
                // Merge: local takes priority over shared
                let merged = ClaudeSettings::merge(&l.settings, &s.settings);
                Ok(Some(SettingsLayer {
                    path: l.path, // Use local path as the representative
                    settings: merged,
                }))
            }
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

    pub fn parsed_permissions(&self) -> Result<ParsedPermissions, SettingsError> {
        let allow = self
            .permissions
            .allow
            .iter()
            .map(|rule| PermissionRule::parse(rule, RuleType::Allow))
            .collect::<Result<Vec<_>, _>>()?;
        let deny = self
            .permissions
            .deny
            .iter()
            .map(|rule| PermissionRule::parse(rule, RuleType::Deny))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ParsedPermissions { allow, deny })
    }

    pub fn merge(high: &ClaudeSettings, low: &ClaudeSettings) -> ClaudeSettings {
        let mut env = low.env.clone();
        env.extend(high.env.clone());

        // Permission lists from each layer are concatenated rather than replaced.
        // Dropping the lower layer's deny rules when the higher layer adds any
        // allow/deny entries would silently weaken security (e.g. a project's
        // allow list erasing the user's global deny list). The deny-before-allow
        // precedence is enforced by `PermissionManager`, not by list order.
        let mut allow = low.permissions.allow.clone();
        allow.extend(high.permissions.allow.iter().cloned());
        let mut deny = low.permissions.deny.clone();
        deny.extend(high.permissions.deny.iter().cloned());

        // Hook lists per event are concatenated (low-layer first, high-layer after).
        let mut hooks: HooksConfig = low.hooks.clone();
        for (event, groups) in &high.hooks {
            hooks
                .entry(event.clone())
                .or_default()
                .extend(groups.iter().cloned());
        }

        // MCP servers: merge by server name, high-priority layer overrides
        // same-name entries from the low-priority layer.
        let mcp_servers = merge_mcp_servers(&high.mcp_servers, &low.mcp_servers);

        ClaudeSettings {
            env,
            model: high.model.clone().or_else(|| low.model.clone()),
            api_key_helper: high
                .api_key_helper
                .clone()
                .or_else(|| low.api_key_helper.clone()),
            permissions: SettingsPermissions { allow, deny },
            hooks,
            mcp_servers,
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
        std::env::remove_var(test_key);
        env.insert(test_key.to_string(), "from-settings".to_string());

        let settings = ClaudeSettings {
            env,
            ..Default::default()
        };
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

        let settings = ClaudeSettings {
            env,
            ..Default::default()
        };
        settings.apply_env();

        assert_eq!(std::env::var(test_key).unwrap(), "from-shell");
        std::env::remove_var(test_key);
    }

    #[test]
    fn test_parse_permissions() {
        let settings = ClaudeSettings {
            permissions: SettingsPermissions {
                allow: vec!["Bash(git status *)".into()],
                deny: vec!["FileWrite".into()],
            },
            ..Default::default()
        };

        let parsed = settings.parsed_permissions().unwrap();
        assert_eq!(parsed.allow.len(), 1);
        assert_eq!(parsed.deny.len(), 1);
        assert_eq!(parsed.allow[0].tool_name, "Bash");
        assert_eq!(parsed.deny[0].tool_name, "FileWrite");
    }

    #[test]
    fn test_discover_project_settings_uses_git_boundary() {
        let root = make_temp_dir("project-settings");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(root.join(".claude/settings.json"), "{}").unwrap();
        let subdir = root.join("nested/app");
        fs::create_dir_all(&subdir).unwrap();

        let found = ClaudeSettings::discover_project_settings(&subdir).unwrap();
        assert!(found.ends_with(".claude/settings.json"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_merge_prefers_high_priority_fields() {
        let mut low_env = HashMap::new();
        low_env.insert("A".into(), "1".into());
        let mut high_env = HashMap::new();
        high_env.insert("B".into(), "2".into());

        let low = ClaudeSettings {
            env: low_env,
            model: Some("low".into()),
            permissions: SettingsPermissions {
                allow: vec!["FileRead".into()],
                deny: vec![],
            },
            ..Default::default()
        };
        let high = ClaudeSettings {
            env: high_env,
            model: Some("high".into()),
            permissions: SettingsPermissions {
                allow: vec!["Bash".into()],
                deny: vec![],
            },
            ..Default::default()
        };

        let merged = ClaudeSettings::merge(&high, &low);
        assert_eq!(merged.model.as_deref(), Some("high"));
        assert_eq!(merged.env.get("A").map(String::as_str), Some("1"));
        assert_eq!(merged.env.get("B").map(String::as_str), Some("2"));
        // Both layers' allow entries are preserved; low-layer first, high-layer after.
        assert_eq!(merged.permissions.allow, vec!["FileRead", "Bash"]);
    }

    #[test]
    fn test_merge_preserves_user_deny_when_project_only_sets_allow() {
        // Regression: a project `.claude/settings.json` that only defines `allow`
        // entries must not drop the user-scope `deny` rules.
        let user = ClaudeSettings {
            permissions: SettingsPermissions {
                allow: vec![],
                deny: vec!["Bash(rm *)".into()],
            },
            ..Default::default()
        };
        let project = ClaudeSettings {
            permissions: SettingsPermissions {
                allow: vec!["Bash(git status *)".into()],
                deny: vec![],
            },
            ..Default::default()
        };

        let merged = ClaudeSettings::merge(&project, &user);
        assert_eq!(merged.permissions.deny, vec!["Bash(rm *)"]);
        assert_eq!(merged.permissions.allow, vec!["Bash(git status *)"]);
    }

    #[test]
    fn test_load_settings_with_hooks() {
        let dir = make_temp_dir("hooks");
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{
                "hooks": {
                    "PreToolUse": [
                        {"matcher": "Bash", "hooks": [{"type": "command", "command": "check.sh"}]}
                    ]
                }
            }"#,
        )
        .unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert_eq!(settings.hooks.len(), 1);
        let groups = &settings.hooks["PreToolUse"];
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(groups[0].hooks.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_settings_without_hooks() {
        let dir = make_temp_dir("no-hooks");
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"model": "claude-sonnet-4-20250514"}"#).unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert!(settings.hooks.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_merge_hooks_same_event() {
        let user = ClaudeSettings {
            hooks: {
                let mut h = HashMap::new();
                h.insert(
                    "PreToolUse".into(),
                    vec![HookEventGroup {
                        matcher: Some("Bash".into()),
                        hooks: vec![],
                    }],
                );
                h
            },
            ..Default::default()
        };
        let project = ClaudeSettings {
            hooks: {
                let mut h = HashMap::new();
                h.insert(
                    "PreToolUse".into(),
                    vec![HookEventGroup {
                        matcher: Some("Write".into()),
                        hooks: vec![],
                    }],
                );
                h
            },
            ..Default::default()
        };

        let merged = ClaudeSettings::merge(&project, &user);
        let groups = &merged.hooks["PreToolUse"];
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(groups[1].matcher.as_deref(), Some("Write"));
    }

    #[test]
    fn test_merge_hooks_different_events() {
        let user = ClaudeSettings {
            hooks: {
                let mut h = HashMap::new();
                h.insert("PreToolUse".into(), vec![]);
                h
            },
            ..Default::default()
        };
        let project = ClaudeSettings {
            hooks: {
                let mut h = HashMap::new();
                h.insert("PostToolUse".into(), vec![]);
                h
            },
            ..Default::default()
        };

        let merged = ClaudeSettings::merge(&project, &user);
        assert!(merged.hooks.contains_key("PreToolUse"));
        assert!(merged.hooks.contains_key("PostToolUse"));
    }

    #[test]
    fn test_load_settings_with_mcp_servers() {
        let dir = make_temp_dir("mcp-servers");
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{
                "mcpServers": {
                    "filesystem": {
                        "type": "stdio",
                        "command": "npx",
                        "args": ["-y", "@anthropic/mcp-server-filesystem"]
                    }
                }
            }"#,
        )
        .unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert_eq!(settings.mcp_servers.len(), 1);
        let fs_server = &settings.mcp_servers["filesystem"];
        assert_eq!(fs_server.command, "npx");
        assert_eq!(
            fs_server.args,
            vec!["-y", "@anthropic/mcp-server-filesystem"]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_settings_without_mcp_servers() {
        let dir = make_temp_dir("no-mcp");
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"model": "claude-sonnet-4-20250514"}"#).unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert!(settings.mcp_servers.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_settings_with_unsupported_mcp_transport() {
        let dir = make_temp_dir("mcp-unsupported");
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{
                "mcpServers": {
                    "remote": {"type": "sse", "command": "http-server"},
                    "local": {"type": "stdio", "command": "npx"}
                }
            }"#,
        )
        .unwrap();

        let settings = ClaudeSettings::load_from(&path).unwrap();
        assert_eq!(settings.mcp_servers.len(), 2);
        // Unsupported transport still loads; filtering happens at runtime
        assert!(!settings.mcp_servers["remote"].is_supported_transport());
        assert!(settings.mcp_servers["local"].is_supported_transport());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_merge_mcp_servers_different_names() {
        use crate::mcp_config::{McpServerConfig, McpTransportType};

        let user = ClaudeSettings {
            mcp_servers: {
                let mut m = HashMap::new();
                m.insert(
                    "github".into(),
                    McpServerConfig {
                        transport_type: McpTransportType::Stdio,
                        command: "gh-mcp".into(),
                        args: vec![],
                        env: HashMap::new(),
                        cwd: None,
                    },
                );
                m
            },
            ..Default::default()
        };
        let project = ClaudeSettings {
            mcp_servers: {
                let mut m = HashMap::new();
                m.insert(
                    "filesystem".into(),
                    McpServerConfig {
                        transport_type: McpTransportType::Stdio,
                        command: "fs-mcp".into(),
                        args: vec![],
                        env: HashMap::new(),
                        cwd: None,
                    },
                );
                m
            },
            ..Default::default()
        };

        let merged = ClaudeSettings::merge(&project, &user);
        assert_eq!(merged.mcp_servers.len(), 2);
        assert!(merged.mcp_servers.contains_key("github"));
        assert!(merged.mcp_servers.contains_key("filesystem"));
    }

    #[test]
    fn test_merge_mcp_servers_project_overrides_user() {
        use crate::mcp_config::{McpServerConfig, McpTransportType};

        let user = ClaudeSettings {
            mcp_servers: {
                let mut m = HashMap::new();
                m.insert(
                    "filesystem".into(),
                    McpServerConfig {
                        transport_type: McpTransportType::Stdio,
                        command: "a".into(),
                        args: vec![],
                        env: HashMap::new(),
                        cwd: None,
                    },
                );
                m
            },
            ..Default::default()
        };
        let project = ClaudeSettings {
            mcp_servers: {
                let mut m = HashMap::new();
                m.insert(
                    "filesystem".into(),
                    McpServerConfig {
                        transport_type: McpTransportType::Stdio,
                        command: "b".into(),
                        args: vec![],
                        env: HashMap::new(),
                        cwd: None,
                    },
                );
                m
            },
            ..Default::default()
        };

        let merged = ClaudeSettings::merge(&project, &user);
        assert_eq!(merged.mcp_servers.len(), 1);
        assert_eq!(merged.mcp_servers["filesystem"].command, "b");
    }

    #[test]
    fn test_merge_mcp_servers_one_layer_empty() {
        use crate::mcp_config::{McpServerConfig, McpTransportType};

        let user = ClaudeSettings {
            mcp_servers: {
                let mut m = HashMap::new();
                m.insert(
                    "github".into(),
                    McpServerConfig {
                        transport_type: McpTransportType::Stdio,
                        command: "gh".into(),
                        args: vec![],
                        env: HashMap::new(),
                        cwd: None,
                    },
                );
                m
            },
            ..Default::default()
        };
        let project = ClaudeSettings::default();

        let merged = ClaudeSettings::merge(&project, &user);
        assert_eq!(merged.mcp_servers.len(), 1);
        assert!(merged.mcp_servers.contains_key("github"));
    }

    #[test]
    fn test_discover_project_local_settings() {
        let root = make_temp_dir("project-local-settings");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.local.json"),
            r#"{"model": "local-model"}"#,
        )
        .unwrap();
        let subdir = root.join("nested/app");
        fs::create_dir_all(&subdir).unwrap();

        let found = ClaudeSettings::discover_project_local_settings(&subdir).unwrap();
        assert!(found.ends_with(".claude/settings.local.json"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_load_project_local_only() {
        let root = make_temp_dir("project-local-only");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.local.json"),
            r#"{"model": "local-model"}"#,
        )
        .unwrap();

        let layer = ClaudeSettings::load_project(&root).unwrap().unwrap();
        assert_eq!(layer.settings.model.as_deref(), Some("local-model"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_load_project_shared_only() {
        let root = make_temp_dir("project-shared-only");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"model": "shared-model"}"#,
        )
        .unwrap();

        let layer = ClaudeSettings::load_project(&root).unwrap().unwrap();
        assert_eq!(layer.settings.model.as_deref(), Some("shared-model"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_load_project_local_overrides_shared() {
        let root = make_temp_dir("project-local-override");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"model": "shared-model", "permissions": {"allow": ["FileRead"], "deny": ["Bash(rm *)"]}}"#,
        )
        .unwrap();
        fs::write(
            root.join(".claude/settings.local.json"),
            r#"{"model": "local-model", "permissions": {"allow": ["FileEdit"], "deny": []}}"#,
        )
        .unwrap();

        let layer = ClaudeSettings::load_project(&root).unwrap().unwrap();
        // Local model overrides shared
        assert_eq!(layer.settings.model.as_deref(), Some("local-model"));
        // Permissions are concatenated across layers
        assert!(layer
            .settings
            .permissions
            .allow
            .contains(&"FileRead".to_string()));
        assert!(layer
            .settings
            .permissions
            .allow
            .contains(&"FileEdit".to_string()));
        // Shared deny rules are preserved
        assert!(layer
            .settings
            .permissions
            .deny
            .contains(&"Bash(rm *)".to_string()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_load_project_no_settings_returns_none() {
        let root = make_temp_dir("project-no-settings");
        fs::create_dir_all(root.join(".git")).unwrap();

        let layer = ClaudeSettings::load_project(&root).unwrap();
        assert!(layer.is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_manual_verification_settings_local_model_override() {
        let root = make_temp_dir("manual-local-model-override");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(
            root.join(".claude/settings.json"),
            r#"{"model": "shared-model"}"#,
        )
        .unwrap();
        fs::write(
            root.join(".claude/settings.local.json"),
            r#"{"model": "local-model"}"#,
        )
        .unwrap();

        let layer = ClaudeSettings::load_project(&root).unwrap().unwrap();
        assert_eq!(layer.settings.model.as_deref(), Some("local-model"));

        let _ = fs::remove_dir_all(&root);
    }
}
