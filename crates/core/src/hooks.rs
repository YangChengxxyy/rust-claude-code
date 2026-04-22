use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Hook Event
// ---------------------------------------------------------------------------

/// The lifecycle events that can trigger hooks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    Stop,
    Notification,
}

impl fmt::Display for HookEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::Stop => "Stop",
            HookEvent::Notification => "Notification",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "PreToolUse" => Some(HookEvent::PreToolUse),
            "PostToolUse" => Some(HookEvent::PostToolUse),
            "UserPromptSubmit" => Some(HookEvent::UserPromptSubmit),
            "Stop" => Some(HookEvent::Stop),
            "Notification" => Some(HookEvent::Notification),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Hook Configuration (deserialized from settings.json)
// ---------------------------------------------------------------------------

/// A single hook definition within an event group.
#[derive(Debug, Clone, Deserialize)]
pub struct HookConfig {
    /// Hook type – only `"command"` is supported in this iteration.
    #[serde(rename = "type")]
    pub type_: String,

    /// Shell command to execute.
    #[serde(default)]
    pub command: Option<String>,

    /// Execution timeout in seconds. Defaults to 10.
    #[serde(default)]
    pub timeout: Option<u64>,
}

/// A group of hooks sharing the same matcher within an event.
#[derive(Debug, Clone, Deserialize)]
pub struct HookEventGroup {
    /// Tool-name matcher for PreToolUse / PostToolUse.
    /// Empty or absent means "match all tools".
    #[serde(default)]
    pub matcher: Option<String>,

    /// The hooks in this group.
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
}

/// Top-level hooks configuration: event name → list of event groups.
pub type HooksConfig = HashMap<String, Vec<HookEventGroup>>;

// ---------------------------------------------------------------------------
// Hook Result (returned by PreToolUse hooks)
// ---------------------------------------------------------------------------

/// The outcome of running hooks for a single event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult {
    /// Execution should continue (tool is approved).
    Continue,
    /// Execution should be blocked.
    Block { reason: String },
}

/// JSON structure expected on stdout from a PreToolUse command hook.
#[derive(Debug, Deserialize)]
pub struct HookCommandResponse {
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Hook Input Structs (serialized to JSON and passed via stdin)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct BaseHookInput {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Debug, Serialize)]
pub struct PreToolUseInput {
    #[serde(flatten)]
    pub base: BaseHookInput,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct PostToolUseInput {
    #[serde(flatten)]
    pub base: BaseHookInput,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_output: String,
    pub tool_is_error: bool,
}

#[derive(Debug, Serialize)]
pub struct UserPromptSubmitInput {
    #[serde(flatten)]
    pub base: BaseHookInput,
    pub user_message: String,
}

#[derive(Debug, Serialize)]
pub struct StopInput {
    #[serde(flatten)]
    pub base: BaseHookInput,
    pub stop_reason: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationInput {
    #[serde(flatten)]
    pub base: BaseHookInput,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_display() {
        assert_eq!(HookEvent::PreToolUse.to_string(), "PreToolUse");
        assert_eq!(HookEvent::PostToolUse.to_string(), "PostToolUse");
        assert_eq!(
            HookEvent::UserPromptSubmit.to_string(),
            "UserPromptSubmit"
        );
        assert_eq!(HookEvent::Stop.to_string(), "Stop");
        assert_eq!(HookEvent::Notification.to_string(), "Notification");
    }

    #[test]
    fn test_hook_event_from_str() {
        assert_eq!(HookEvent::from_str("PreToolUse"), Some(HookEvent::PreToolUse));
        assert_eq!(HookEvent::from_str("PostToolUse"), Some(HookEvent::PostToolUse));
        assert_eq!(
            HookEvent::from_str("UserPromptSubmit"),
            Some(HookEvent::UserPromptSubmit)
        );
        assert_eq!(HookEvent::from_str("Stop"), Some(HookEvent::Stop));
        assert_eq!(HookEvent::from_str("Notification"), Some(HookEvent::Notification));
        assert_eq!(HookEvent::from_str("SubagentStart"), None);
        assert_eq!(HookEvent::from_str("unknown"), None);
    }

    #[test]
    fn test_hook_config_deserialize_minimal() {
        let json = r#"{"type": "command", "command": "echo ok"}"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.type_, "command");
        assert_eq!(config.command.as_deref(), Some("echo ok"));
        assert_eq!(config.timeout, None);
    }

    #[test]
    fn test_hook_config_deserialize_full() {
        let json = r#"{"type": "command", "command": "/usr/local/bin/check.sh", "timeout": 30}"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.type_, "command");
        assert_eq!(config.command.as_deref(), Some("/usr/local/bin/check.sh"));
        assert_eq!(config.timeout, Some(30));
    }

    #[test]
    fn test_hook_config_deserialize_unsupported_type() {
        let json = r#"{"type": "prompt", "command": "check $ARGUMENTS"}"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.type_, "prompt");
    }

    #[test]
    fn test_hook_event_group_deserialize() {
        let json = r#"{"matcher": "Bash", "hooks": [{"type": "command", "command": "check.sh"}]}"#;
        let group: HookEventGroup = serde_json::from_str(json).unwrap();
        assert_eq!(group.matcher.as_deref(), Some("Bash"));
        assert_eq!(group.hooks.len(), 1);
        assert_eq!(group.hooks[0].type_, "command");
    }

    #[test]
    fn test_hook_event_group_empty_matcher() {
        let json = r#"{"matcher": "", "hooks": [{"type": "command", "command": "log.sh"}]}"#;
        let group: HookEventGroup = serde_json::from_str(json).unwrap();
        assert_eq!(group.matcher.as_deref(), Some(""));
    }

    #[test]
    fn test_hooks_config_deserialize() {
        let json = r#"{
            "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "check.sh"}]}],
            "PostToolUse": [{"hooks": [{"type": "command", "command": "log.sh"}]}]
        }"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.len(), 2);
        assert!(config.contains_key("PreToolUse"));
        assert!(config.contains_key("PostToolUse"));
    }

    #[test]
    fn test_hook_result_eq() {
        assert_eq!(HookResult::Continue, HookResult::Continue);
        assert_eq!(
            HookResult::Block {
                reason: "bad".into()
            },
            HookResult::Block {
                reason: "bad".into()
            }
        );
        assert_ne!(HookResult::Continue, HookResult::Block { reason: "x".into() });
    }

    #[test]
    fn test_hook_command_response_deserialize() {
        let json = r#"{"decision": "block", "reason": "unsafe"}"#;
        let resp: HookCommandResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.decision.as_deref(), Some("block"));
        assert_eq!(resp.reason.as_deref(), Some("unsafe"));
    }

    #[test]
    fn test_hook_command_response_empty() {
        let json = r#"{}"#;
        let resp: HookCommandResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.decision, None);
        assert_eq!(resp.reason, None);
    }

    #[test]
    fn test_pre_tool_use_input_serialize() {
        let input = PreToolUseInput {
            base: BaseHookInput {
                session_id: "sess-1".into(),
                cwd: "/tmp".into(),
            },
            tool_name: "Bash".into(),
            tool_input: serde_json::json!({"command": "ls"}),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["session_id"], "sess-1");
        assert_eq!(json["cwd"], "/tmp");
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_input"]["command"], "ls");
    }

    #[test]
    fn test_post_tool_use_input_serialize() {
        let input = PostToolUseInput {
            base: BaseHookInput {
                session_id: "".into(),
                cwd: "/home".into(),
            },
            tool_name: "FileRead".into(),
            tool_input: serde_json::json!({"file_path": "/etc/hosts"}),
            tool_output: "127.0.0.1 localhost".into(),
            tool_is_error: false,
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["tool_name"], "FileRead");
        assert_eq!(json["tool_is_error"], false);
    }

    #[test]
    fn test_user_prompt_submit_input_serialize() {
        let input = UserPromptSubmitInput {
            base: BaseHookInput {
                session_id: "s".into(),
                cwd: "/".into(),
            },
            user_message: "write a test".into(),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["user_message"], "write a test");
    }

    #[test]
    fn test_stop_input_serialize() {
        let input = StopInput {
            base: BaseHookInput {
                session_id: "s".into(),
                cwd: "/".into(),
            },
            stop_reason: "end_turn".into(),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["stop_reason"], "end_turn");
    }
}
