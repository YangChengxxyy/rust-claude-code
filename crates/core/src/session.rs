use serde::{Deserialize, Serialize};

use crate::message::{Message, Usage};
use crate::permission::{PermissionMode, PermissionRule};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub model: String,
    pub model_setting: String,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub first_user_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_usage: Option<Usage>,
    /// Whether this session was interrupted (crashed without a `SessionEnd` event).
    #[serde(default, skip_serializing_if = "is_false")]
    pub interrupted: bool,
}

fn is_false(v: &bool) -> bool {
    !v
}

/// A structured log entry in a JSONL session file.
///
/// Each variant serializes as a JSON object with a `"type"` tag field,
/// e.g. `{"type": "header", "id": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// First line of a JSONL session file — session metadata.
    Header {
        id: String,
        model: String,
        model_setting: String,
        cwd: String,
        created_at: String,
    },
    /// A user message appended to the session.
    UserMessage { message: Message },
    /// An assistant message appended to the session.
    AssistantMessage { message: Message },
    /// Marks a compaction boundary — messages before this point were summarized.
    CompactBoundary { summary: String },
    /// Cumulative token usage update.
    UsageUpdate { usage: Usage },
    /// Permission mode or rules changed during the session.
    PermissionChange {
        mode: PermissionMode,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        allow_rules: Vec<PermissionRule>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        deny_rules: Vec<PermissionRule>,
    },
    /// Normal session termination marker.
    SessionEnd { updated_at: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSnapshot {
    pub model: String,
    pub context_capacity: Option<u32>,
    pub used_tokens: u32,
    pub system_prompt_tokens: u32,
    pub message_tokens: u32,
    pub tool_result_tokens: u32,
    pub remaining_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ContentBlock;
    use crate::permission::RuleType;

    fn roundtrip(event: &SessionEvent) -> SessionEvent {
        let json = serde_json::to_string(event).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_header_event_roundtrip() {
        let event = SessionEvent::Header {
            id: "20260504_143022".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            model_setting: "sonnet".to_string(),
            cwd: "/tmp/project".to_string(),
            created_at: "2026-05-04T14:30:22+08:00".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"header\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_user_message_event_roundtrip() {
        let event = SessionEvent::UserMessage {
            message: Message::user("hello"),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"user_message\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_assistant_message_event_roundtrip() {
        let event = SessionEvent::AssistantMessage {
            message: Message::assistant_with_usage(
                vec![ContentBlock::text("hi there")],
                Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            ),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"assistant_message\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_compact_boundary_event_roundtrip() {
        let event = SessionEvent::CompactBoundary {
            summary: "User discussed project setup and file structure.".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"compact_boundary\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_usage_update_event_roundtrip() {
        let event = SessionEvent::UsageUpdate {
            usage: Usage {
                input_tokens: 50000,
                output_tokens: 10000,
                cache_creation_input_tokens: 5000,
                cache_read_input_tokens: 2000,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"usage_update\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_permission_change_event_roundtrip() {
        let event = SessionEvent::PermissionChange {
            mode: PermissionMode::AcceptEdits,
            allow_rules: vec![PermissionRule {
                tool_name: "Bash".to_string(),
                pattern: Some("git *".to_string()),
                path_pattern: None,
                rule_type: RuleType::Allow,
            }],
            deny_rules: vec![],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"permission_change\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_permission_change_event_omits_empty_rules() {
        let event = SessionEvent::PermissionChange {
            mode: PermissionMode::Plan,
            allow_rules: vec![],
            deny_rules: vec![],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("allow_rules"));
        assert!(!json.contains("deny_rules"));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_session_end_event_roundtrip() {
        let event = SessionEvent::SessionEnd {
            updated_at: "2026-05-04T15:00:00+08:00".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"session_end\""));
        assert_eq!(roundtrip(&event), event);
    }

    #[test]
    fn test_unknown_event_type_is_tolerated_via_value() {
        // When reading JSONL, unknown types should be skippable.
        // We test that serde_json::Value can parse unknown types and
        // SessionEvent deserialization fails gracefully (used by reader to skip).
        let json = r#"{"type":"future_event","data":"something"}"#;
        let result: Result<SessionEvent, _> = serde_json::from_str(json);
        assert!(result.is_err()); // This is expected — the reader will catch and skip
    }

    #[test]
    fn test_session_summary_interrupted_default_false() {
        let json = r#"{
            "id": "test",
            "model": "m",
            "model_setting": "m",
            "cwd": "/",
            "created_at": "t",
            "updated_at": "t",
            "message_count": 0,
            "first_user_summary": ""
        }"#;
        let summary: SessionSummary = serde_json::from_str(json).unwrap();
        assert!(!summary.interrupted);
    }

    #[test]
    fn test_session_summary_interrupted_true_roundtrip() {
        let summary = SessionSummary {
            id: "test".to_string(),
            model: "m".to_string(),
            model_setting: "m".to_string(),
            cwd: "/".to_string(),
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            message_count: 5,
            first_user_summary: "hello".to_string(),
            total_usage: None,
            interrupted: true,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"interrupted\":true"));
        let parsed: SessionSummary = serde_json::from_str(&json).unwrap();
        assert!(parsed.interrupted);
    }

    #[test]
    fn test_session_summary_not_interrupted_omits_field() {
        let summary = SessionSummary {
            id: "test".to_string(),
            model: "m".to_string(),
            model_setting: "m".to_string(),
            cwd: "/".to_string(),
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
            message_count: 0,
            first_user_summary: "".to_string(),
            total_usage: None,
            interrupted: false,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("interrupted"));
    }
}
