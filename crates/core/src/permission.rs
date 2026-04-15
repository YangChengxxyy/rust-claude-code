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
}
