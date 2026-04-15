use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionRequest<'a> {
    pub tool_name: &'a str,
    pub command: Option<&'a str>,
    pub is_read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
    DontAsk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub tool_name: String,
    #[serde(default)]
    pub pattern: Option<String>,
    pub rule_type: RuleType,
}

impl PermissionRule {
    /// Parse a permission rule string like "Bash", "Bash(git *)", or "FileRead".
    /// The `rule_type` is determined by the caller context (allow vs deny).
    pub fn parse(s: &str, rule_type: RuleType) -> Result<PermissionRule, PermissionError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(PermissionError::Parse("empty rule string".to_string()));
        }

        if let Some(paren_start) = s.find('(') {
            let tool_name = s[..paren_start].trim().to_string();
            if tool_name.is_empty() {
                return Err(PermissionError::Parse(
                    "empty tool name before '('".to_string(),
                ));
            }
            if !s.ends_with(')') {
                return Err(PermissionError::Parse(
                    "missing closing ')' in rule".to_string(),
                ));
            }
            let pattern = s[paren_start + 1..s.len() - 1].to_string();
            Ok(PermissionRule {
                tool_name,
                pattern: Some(pattern),
                rule_type,
            })
        } else {
            Ok(PermissionRule {
                tool_name: s.to_string(),
                pattern: None,
                rule_type,
            })
        }
    }

    /// Format a rule as a compact string like "Bash" or "Bash(git *)".
    pub fn to_compact_string(&self) -> String {
        match &self.pattern {
            Some(pattern) => format!("{}({})", self.tool_name, pattern),
            None => self.tool_name.clone(),
        }
    }
}

/// Errors from the permission subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    #[error("failed to parse permission rule: {0}")]
    Parse(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Manages permission state: the current mode plus allow/deny rule lists.
/// Provides check, persistence (load/save), and rule manipulation.
#[derive(Debug, Clone)]
pub struct PermissionManager {
    pub mode: PermissionMode,
    pub always_allow: Vec<PermissionRule>,
    pub always_deny: Vec<PermissionRule>,
}

/// JSON representation for persistence.
#[derive(Debug, Serialize, Deserialize)]
struct PermissionManagerFile {
    mode: String,
    allow: Vec<String>,
    deny: Vec<String>,
}

impl PermissionManager {
    pub fn new(mode: PermissionMode) -> Self {
        PermissionManager {
            mode,
            always_allow: Vec::new(),
            always_deny: Vec::new(),
        }
    }

    pub fn check_permission(&self, request: PermissionRequest<'_>) -> PermissionCheck {
        self.mode
            .check(request, &self.always_deny, &self.always_allow)
    }

    pub fn add_allow_rule(&mut self, rule: PermissionRule) {
        self.always_allow.push(rule);
    }

    pub fn add_deny_rule(&mut self, rule: PermissionRule) {
        self.always_deny.push(rule);
    }

    /// Load a `PermissionManager` from a JSON file.
    pub fn load(path: &Path) -> Result<Self, PermissionError> {
        let content = std::fs::read_to_string(path)?;
        let file: PermissionManagerFile = serde_json::from_str(&content)?;

        let mode = mode_from_str(&file.mode)?;
        let always_allow = file
            .allow
            .iter()
            .map(|s| PermissionRule::parse(s, RuleType::Allow))
            .collect::<Result<Vec<_>, _>>()?;
        let always_deny = file
            .deny
            .iter()
            .map(|s| PermissionRule::parse(s, RuleType::Deny))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PermissionManager {
            mode,
            always_allow,
            always_deny,
        })
    }

    /// Persist the current state to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), PermissionError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = PermissionManagerFile {
            mode: mode_to_str(self.mode),
            allow: self
                .always_allow
                .iter()
                .map(|r| r.to_compact_string())
                .collect(),
            deny: self
                .always_deny
                .iter()
                .map(|r| r.to_compact_string())
                .collect(),
        };

        let content = serde_json::to_string_pretty(&file)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Default path for persisted permissions: `~/.config/rust-claude-code/permissions.json`.
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("rust-claude-code")
            .join("permissions.json")
    }
}

fn mode_to_str(mode: PermissionMode) -> String {
    match mode {
        PermissionMode::Default => "default".to_string(),
        PermissionMode::AcceptEdits => "accept-edits".to_string(),
        PermissionMode::BypassPermissions => "bypass".to_string(),
        PermissionMode::Plan => "plan".to_string(),
        PermissionMode::DontAsk => "dont-ask".to_string(),
    }
}

fn mode_from_str(s: &str) -> Result<PermissionMode, PermissionError> {
    match s {
        "default" => Ok(PermissionMode::Default),
        "accept-edits" => Ok(PermissionMode::AcceptEdits),
        "bypass" => Ok(PermissionMode::BypassPermissions),
        "plan" => Ok(PermissionMode::Plan),
        "dont-ask" => Ok(PermissionMode::DontAsk),
        other => Err(PermissionError::Parse(format!(
            "unknown permission mode: {other}"
        ))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionCheck {
    Allowed,
    Denied { reason: String },
    NeedsConfirmation { prompt: String },
}

impl PermissionMode {
    pub fn check(
        &self,
        request: PermissionRequest<'_>,
        always_deny: &[PermissionRule],
        always_allow: &[PermissionRule],
    ) -> PermissionCheck {
        if let Some(check) = match_rule(request, always_deny) {
            return check;
        }

        if let Some(check) = match_rule(request, always_allow) {
            if matches!(check, PermissionCheck::Allowed)
                && matches!(self, PermissionMode::Plan)
                && !request.is_read_only
            {
                return PermissionCheck::Denied {
                    reason: "Plan mode does not allow write operations".to_string(),
                };
            }

            return check;
        }

        self.check_tool_with_command(
            request.tool_name,
            request.command,
            request.is_read_only,
            &[],
        )
    }

    pub fn check_tool(
        &self,
        tool_name: &str,
        is_read_only: bool,
        rules: &[PermissionRule],
    ) -> PermissionCheck {
        debug_assert!(
            rules.iter().all(|rule| rule.pattern.is_none()),
            "pattern-based permission rules are not supported by check_tool(); use check_tool_with_command() for command-aware checks"
        );

        for rule in rules {
            if rule.tool_name == tool_name || rule.tool_name == "*" {
                if rule.pattern.is_none() {
                    return rule.rule_type.to_permission_check();
                }
            }
        }

        match self {
            PermissionMode::BypassPermissions => PermissionCheck::Allowed,
            PermissionMode::Plan => {
                if is_read_only {
                    PermissionCheck::Allowed
                } else {
                    PermissionCheck::Denied {
                        reason: "Plan mode does not allow write operations".to_string(),
                    }
                }
            }
            PermissionMode::AcceptEdits => {
                if is_read_only || tool_name == "FileEdit" || tool_name == "FileWrite" {
                    PermissionCheck::Allowed
                } else {
                    PermissionCheck::NeedsConfirmation {
                        prompt: format!("Allow {} to execute?", tool_name),
                    }
                }
            }
            PermissionMode::DontAsk => PermissionCheck::Denied {
                reason: "Permission denied (dontAsk mode)".to_string(),
            },
            PermissionMode::Default => {
                if is_read_only {
                    PermissionCheck::Allowed
                } else {
                    PermissionCheck::NeedsConfirmation {
                        prompt: format!("Allow {} to execute?", tool_name),
                    }
                }
            }
        }
    }

    pub fn check_tool_with_command(
        &self,
        tool_name: &str,
        command: Option<&str>,
        is_read_only: bool,
        rules: &[PermissionRule],
    ) -> PermissionCheck {
        for rule in rules {
            if rule.tool_name != tool_name && rule.tool_name != "*" {
                continue;
            }

            let pattern_matches = match (&rule.pattern, command) {
                (None, _) => true,
                (Some(pattern), Some(command)) => pattern_matches(command, pattern),
                (Some(_), None) => false,
            };

            if pattern_matches {
                return rule.rule_type.to_permission_check();
            }
        }

        self.check_tool(tool_name, is_read_only, &[])
    }
}

impl RuleType {
    fn to_permission_check(self) -> PermissionCheck {
        match self {
            RuleType::Allow => PermissionCheck::Allowed,
            RuleType::Deny => PermissionCheck::Denied {
                reason: "Denied by rule".to_string(),
            },
        }
    }
}

fn match_rule(request: PermissionRequest<'_>, rules: &[PermissionRule]) -> Option<PermissionCheck> {
    for rule in rules {
        if rule.tool_name != request.tool_name && rule.tool_name != "*" {
            continue;
        }

        let pattern_matches = match (&rule.pattern, request.command) {
            (None, _) => true,
            (Some(pattern), Some(command)) => pattern_matches(command, pattern),
            (Some(_), None) => false,
        };

        if pattern_matches {
            return Some(rule.rule_type.to_permission_check());
        }
    }

    None
}

fn pattern_matches(value: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }

    value == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mode_serde() {
        let json = serde_json::to_string(&PermissionMode::BypassPermissions).unwrap();
        assert_eq!(json, "\"bypassPermissions\"");

        let mode: PermissionMode = serde_json::from_str("\"plan\"").unwrap();
        assert_eq!(mode, PermissionMode::Plan);
    }

    #[test]
    fn test_bypass_permissions_allows_all() {
        let check = PermissionMode::BypassPermissions.check_tool("Bash", false, &[]);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_plan_mode_allows_readonly() {
        let check = PermissionMode::Plan.check_tool("FileRead", true, &[]);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_plan_mode_denies_writes() {
        let check = PermissionMode::Plan.check_tool("Bash", false, &[]);
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_default_mode_needs_confirmation_for_writes() {
        let check = PermissionMode::Default.check_tool("Bash", false, &[]);
        assert!(matches!(check, PermissionCheck::NeedsConfirmation { .. }));
    }

    #[test]
    fn test_default_mode_allows_readonly() {
        let check = PermissionMode::Default.check_tool("FileRead", true, &[]);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_accept_edits_allows_file_operations() {
        let check = PermissionMode::AcceptEdits.check_tool("FileEdit", false, &[]);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_accept_edits_needs_confirmation_for_bash() {
        let check = PermissionMode::AcceptEdits.check_tool("Bash", false, &[]);
        assert!(matches!(check, PermissionCheck::NeedsConfirmation { .. }));
    }

    #[test]
    fn test_dont_ask_denies_all() {
        let check = PermissionMode::DontAsk.check_tool("Bash", false, &[]);
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_always_allow_rule() {
        let rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: None,
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Default.check_tool("Bash", false, &rules);
        assert!(matches!(check, PermissionCheck::Allowed));
    }

    #[test]
    fn test_always_deny_rule() {
        let rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: None,
            rule_type: RuleType::Deny,
        }];

        let check = PermissionMode::BypassPermissions.check_tool("Bash", false, &rules);
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_pattern_rule_matches_command() {
        let rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Default.check_tool_with_command(
            "Bash",
            Some("git status"),
            false,
            &rules,
        );
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_pattern_rule_is_ignored_when_command_does_not_match() {
        let rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Default.check_tool_with_command(
            "Bash",
            Some("rm -rf /tmp/demo"),
            false,
            &rules,
        );
        assert!(matches!(check, PermissionCheck::NeedsConfirmation { .. }));
    }

    #[test]
    fn test_wildcard_rule() {
        let rules = vec![PermissionRule {
            tool_name: "*".to_string(),
            pattern: None,
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Default.check_tool("Bash", false, &rules);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_pattern_matching() {
        assert!(pattern_matches("git status", "git *"));
        assert!(pattern_matches("git commit -m 'test'", "git *"));
        assert!(!pattern_matches("rm -rf /", "git *"));
        assert!(pattern_matches("anything", "*"));
    }

    #[test]
    fn test_check_prioritizes_deny_over_allow() {
        let request = PermissionRequest {
            tool_name: "Bash",
            command: Some("git status"),
            is_read_only: false,
        };

        let allow_rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        }];
        let deny_rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git status".to_string()),
            rule_type: RuleType::Deny,
        }];

        let check = PermissionMode::Default.check(request, &deny_rules, &allow_rules);
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_check_plan_mode_does_not_allow_write_even_with_allow_rule() {
        let request = PermissionRequest {
            tool_name: "FileWrite",
            command: None,
            is_read_only: false,
        };

        let allow_rules = vec![PermissionRule {
            tool_name: "FileWrite".to_string(),
            pattern: None,
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Plan.check(request, &[], &allow_rules);
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_check_plan_mode_still_allows_read_only_allow_rule() {
        let request = PermissionRequest {
            tool_name: "FileRead",
            command: None,
            is_read_only: true,
        };

        let allow_rules = vec![PermissionRule {
            tool_name: "FileRead".to_string(),
            pattern: None,
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Plan.check(request, &[], &allow_rules);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_check_pattern_rule_without_command_falls_back_to_mode() {
        let request = PermissionRequest {
            tool_name: "Bash",
            command: None,
            is_read_only: false,
        };

        let allow_rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Default.check(request, &[], &allow_rules);
        assert!(matches!(check, PermissionCheck::NeedsConfirmation { .. }));
    }

    #[test]
    fn test_check_wildcard_pattern_rule_matches_command() {
        let request = PermissionRequest {
            tool_name: "Bash",
            command: Some("git status"),
            is_read_only: false,
        };

        let allow_rules = vec![PermissionRule {
            tool_name: "*".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        }];

        let check = PermissionMode::Default.check(request, &[], &allow_rules);
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_permission_rule_serde() {
        let rule = PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        };
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: PermissionRule = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool_name, "Bash");
        assert_eq!(parsed.pattern, Some("git *".to_string()));
    }

    // --- PermissionRule::parse tests ---

    #[test]
    fn test_parse_simple_tool_name() {
        let rule = PermissionRule::parse("Bash", RuleType::Allow).unwrap();
        assert_eq!(rule.tool_name, "Bash");
        assert_eq!(rule.pattern, None);
        assert_eq!(rule.rule_type, RuleType::Allow);
    }

    #[test]
    fn test_parse_tool_with_pattern() {
        let rule = PermissionRule::parse("Bash(git *)", RuleType::Allow).unwrap();
        assert_eq!(rule.tool_name, "Bash");
        assert_eq!(rule.pattern, Some("git *".to_string()));
        assert_eq!(rule.rule_type, RuleType::Allow);
    }

    #[test]
    fn test_parse_tool_name_only_no_parens() {
        let rule = PermissionRule::parse("FileRead", RuleType::Deny).unwrap();
        assert_eq!(rule.tool_name, "FileRead");
        assert_eq!(rule.pattern, None);
        assert_eq!(rule.rule_type, RuleType::Deny);
    }

    #[test]
    fn test_parse_with_complex_pattern() {
        let rule = PermissionRule::parse("Bash(rm -rf /)", RuleType::Deny).unwrap();
        assert_eq!(rule.tool_name, "Bash");
        assert_eq!(rule.pattern, Some("rm -rf /".to_string()));
    }

    #[test]
    fn test_parse_empty_string_fails() {
        assert!(PermissionRule::parse("", RuleType::Allow).is_err());
    }

    #[test]
    fn test_parse_missing_closing_paren_fails() {
        assert!(PermissionRule::parse("Bash(git *", RuleType::Allow).is_err());
    }

    #[test]
    fn test_parse_empty_tool_name_before_paren_fails() {
        assert!(PermissionRule::parse("(git *)", RuleType::Allow).is_err());
    }

    #[test]
    fn test_compact_string_simple() {
        let rule = PermissionRule {
            tool_name: "FileRead".to_string(),
            pattern: None,
            rule_type: RuleType::Allow,
        };
        assert_eq!(rule.to_compact_string(), "FileRead");
    }

    #[test]
    fn test_compact_string_with_pattern() {
        let rule = PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        };
        assert_eq!(rule.to_compact_string(), "Bash(git *)");
    }

    #[test]
    fn test_parse_roundtrip() {
        let original = "Bash(git *)";
        let rule = PermissionRule::parse(original, RuleType::Allow).unwrap();
        assert_eq!(rule.to_compact_string(), original);
    }

    // --- PermissionManager tests ---

    #[test]
    fn test_permission_manager_new() {
        let manager = PermissionManager::new(PermissionMode::Default);
        assert_eq!(manager.mode, PermissionMode::Default);
        assert!(manager.always_allow.is_empty());
        assert!(manager.always_deny.is_empty());
    }

    #[test]
    fn test_permission_manager_add_rules() {
        let mut manager = PermissionManager::new(PermissionMode::Default);
        manager.add_allow_rule(PermissionRule::parse("Bash(git *)", RuleType::Allow).unwrap());
        manager.add_deny_rule(PermissionRule::parse("Bash(rm -rf /)", RuleType::Deny).unwrap());
        assert_eq!(manager.always_allow.len(), 1);
        assert_eq!(manager.always_deny.len(), 1);
    }

    #[test]
    fn test_permission_manager_check_allowed() {
        let mut manager = PermissionManager::new(PermissionMode::Default);
        manager.add_allow_rule(PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        });

        let check = manager.check_permission(PermissionRequest {
            tool_name: "Bash",
            command: Some("git status"),
            is_read_only: false,
        });
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_permission_manager_check_denied() {
        let mut manager = PermissionManager::new(PermissionMode::Default);
        manager.add_deny_rule(PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: None,
            rule_type: RuleType::Deny,
        });

        let check = manager.check_permission(PermissionRequest {
            tool_name: "Bash",
            command: Some("ls"),
            is_read_only: false,
        });
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_permission_manager_check_needs_confirmation() {
        let manager = PermissionManager::new(PermissionMode::Default);

        let check = manager.check_permission(PermissionRequest {
            tool_name: "Bash",
            command: Some("ls"),
            is_read_only: false,
        });
        assert!(matches!(check, PermissionCheck::NeedsConfirmation { .. }));
    }

    #[test]
    fn test_permission_manager_bypass_mode() {
        let manager = PermissionManager::new(PermissionMode::BypassPermissions);

        let check = manager.check_permission(PermissionRequest {
            tool_name: "Bash",
            command: Some("rm -rf /tmp/test"),
            is_read_only: false,
        });
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_permission_manager_plan_mode_denies_write() {
        let manager = PermissionManager::new(PermissionMode::Plan);

        let check = manager.check_permission(PermissionRequest {
            tool_name: "FileWrite",
            command: None,
            is_read_only: false,
        });
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_permission_manager_plan_mode_allows_read() {
        let manager = PermissionManager::new(PermissionMode::Plan);

        let check = manager.check_permission(PermissionRequest {
            tool_name: "FileRead",
            command: None,
            is_read_only: true,
        });
        assert_eq!(check, PermissionCheck::Allowed);
    }

    #[test]
    fn test_permission_manager_save_and_load_roundtrip() {
        let temp_dir =
            std::env::temp_dir().join(format!("perm-manager-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("permissions.json");

        let mut manager = PermissionManager::new(PermissionMode::AcceptEdits);
        manager.add_allow_rule(PermissionRule::parse("Bash(git *)", RuleType::Allow).unwrap());
        manager.add_allow_rule(PermissionRule::parse("FileRead", RuleType::Allow).unwrap());
        manager.add_deny_rule(PermissionRule::parse("Bash(rm -rf /)", RuleType::Deny).unwrap());

        manager.save(&path).unwrap();

        let loaded = PermissionManager::load(&path).unwrap();
        assert_eq!(loaded.mode, PermissionMode::AcceptEdits);
        assert_eq!(loaded.always_allow.len(), 2);
        assert_eq!(loaded.always_deny.len(), 1);
        assert_eq!(loaded.always_allow[0].tool_name, "Bash");
        assert_eq!(loaded.always_allow[0].pattern, Some("git *".to_string()));
        assert_eq!(loaded.always_allow[1].tool_name, "FileRead");
        assert_eq!(loaded.always_allow[1].pattern, None);
        assert_eq!(loaded.always_deny[0].tool_name, "Bash");
        assert_eq!(
            loaded.always_deny[0].pattern,
            Some("rm -rf /".to_string())
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_permission_manager_save_creates_parent_dirs() {
        let temp_dir =
            std::env::temp_dir().join(format!("perm-manager-mkdir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let path = temp_dir.join("nested").join("dir").join("permissions.json");

        let manager = PermissionManager::new(PermissionMode::Default);
        manager.save(&path).unwrap();
        assert!(path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_permission_manager_load_nonexistent_file() {
        let path = std::env::temp_dir().join("nonexistent-perm-file.json");
        assert!(PermissionManager::load(&path).is_err());
    }

    #[test]
    fn test_permission_manager_default_path() {
        let path = PermissionManager::default_path();
        assert!(path.ends_with("permissions.json"));
        assert!(path.to_string_lossy().contains("rust-claude-code"));
    }

    #[test]
    fn test_mode_string_roundtrip() {
        let modes = [
            PermissionMode::Default,
            PermissionMode::AcceptEdits,
            PermissionMode::BypassPermissions,
            PermissionMode::Plan,
            PermissionMode::DontAsk,
        ];

        for mode in modes {
            let s = mode_to_str(mode);
            let parsed = mode_from_str(&s).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_mode_from_str_unknown_fails() {
        assert!(mode_from_str("unknown-mode").is_err());
    }

    #[test]
    fn test_permission_manager_deny_overrides_allow() {
        let mut manager = PermissionManager::new(PermissionMode::Default);
        manager.add_allow_rule(PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: RuleType::Allow,
        });
        manager.add_deny_rule(PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git status".to_string()),
            rule_type: RuleType::Deny,
        });

        let check = manager.check_permission(PermissionRequest {
            tool_name: "Bash",
            command: Some("git status"),
            is_read_only: false,
        });
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_permission_manager_check_delegates_to_mode() {
        // DontAsk mode denies everything
        let manager = PermissionManager::new(PermissionMode::DontAsk);

        let check = manager.check_permission(PermissionRequest {
            tool_name: "FileRead",
            command: None,
            is_read_only: true,
        });
        // DontAsk falls through to mode default which denies
        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }
}
