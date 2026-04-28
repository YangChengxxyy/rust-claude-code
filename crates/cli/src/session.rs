//! Session persistence — save and load conversation history.
//!
//! Sessions are stored as JSON files under `~/.config/rust-claude-code/sessions/`.
//! Each session file contains the message history, model info, and metadata.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use rust_claude_core::message::{ContentBlock, Message, Role, Usage};
use rust_claude_core::model::get_runtime_main_loop_model;
use rust_claude_core::permission::{PermissionMode, PermissionRule};
use rust_claude_core::session::SessionSummary;
use rust_claude_core::state::AppState;

/// Metadata and message history for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    /// Session identifier (timestamp-based).
    pub id: String,
    /// Model used in this session.
    pub model: String,
    /// Original user-specified model setting for this session.
    #[serde(default)]
    pub model_setting: String,
    /// Working directory when the session was created.
    pub cwd: String,
    /// When the session was created (ISO 8601).
    pub created_at: String,
    /// When the session was last updated (ISO 8601).
    pub updated_at: String,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Accumulated token usage across the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_usage: Option<Usage>,
    /// Permission mode active during the session.
    #[serde(default)]
    pub permission_mode: PermissionMode,
    /// Always-allow rules accumulated during the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always_allow_rules: Vec<PermissionRule>,
    /// Always-deny rules accumulated during the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always_deny_rules: Vec<PermissionRule>,
}

impl SessionFile {
    /// Create a new session with the given model and working directory.
    pub fn new(model: &str, model_setting: &str, cwd: &Path) -> Self {
        let now = chrono::Local::now().to_rfc3339();
        let id = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        SessionFile {
            id,
            model: model.to_string(),
            model_setting: model_setting.to_string(),
            cwd: cwd.display().to_string(),
            created_at: now.clone(),
            updated_at: now,
            messages: Vec::new(),
            total_usage: None,
            permission_mode: PermissionMode::Default,
            always_allow_rules: Vec::new(),
            always_deny_rules: Vec::new(),
        }
    }

    /// Save this session to its file.
    pub fn save(&mut self) -> Result<PathBuf> {
        self.updated_at = chrono::Local::now().to_rfc3339();
        let dir = sessions_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create sessions directory: {}", dir.display()))?;
        let path = dir.join(format!("{}.json", self.id));
        let json = serde_json::to_string_pretty(self).context("failed to serialize session")?;
        std::fs::write(&path, json)
            .with_context(|| format!("failed to write session file: {}", path.display()))?;
        Ok(path)
    }

    /// Load a session from a file path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read session file: {}", path.display()))?;
        let mut session: SessionFile = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse session file: {}", path.display()))?;
        if session.model_setting.is_empty() {
            session.model_setting = session.model.clone();
        }
        Ok(session)
    }

    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id.clone(),
            model: self.model.clone(),
            model_setting: if self.model_setting.is_empty() {
                self.model.clone()
            } else {
                self.model_setting.clone()
            },
            cwd: self.cwd.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            message_count: self.messages.len(),
            first_user_summary: first_user_summary(&self.messages),
            total_usage: self.total_usage.clone(),
        }
    }
}

fn first_user_summary(messages: &[Message]) -> String {
    let Some(message) = messages.iter().find(|message| message.role == Role::User) else {
        return "(no user message)".to_string();
    };

    let text = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    summarize_text(&text)
}

fn summarize_text(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "(non-text user message)".to_string();
    }

    const MAX_CHARS: usize = 80;
    let mut chars = collapsed.chars();
    let summary: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

pub fn restore_app_state_from_session(state: &mut AppState, prev: &SessionFile) {
    state.messages = prev.messages.clone();
    state.session.id = prev.id.clone();
    if !prev.model_setting.is_empty() {
        state.session.model_setting = prev.model_setting.clone();
    } else {
        state.session.model_setting = prev.model.clone();
    }
    if let Some(usage) = &prev.total_usage {
        state.total_usage = usage.clone();
    }
    state.permission_mode = prev.permission_mode;
    if !prev.always_allow_rules.is_empty() {
        state.always_allow_rules = prev.always_allow_rules.clone();
    }
    if !prev.always_deny_rules.is_empty() {
        state.always_deny_rules = prev.always_deny_rules.clone();
    }
    state.session.model =
        get_runtime_main_loop_model(&state.session.model_setting, state.permission_mode, false);
}

/// Return the sessions directory: `~/.config/rust-claude-code/sessions/`.
pub fn sessions_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("rust-claude-code")
        .join("sessions")
}

/// Load the most recent session file, if any.
pub fn load_latest_session() -> Result<Option<SessionFile>> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Ok(None);
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .context("failed to read sessions directory")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        return Ok(None);
    }

    // Sort by filename (which is timestamp-based) in descending order
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    let latest_path = entries[0].path();
    let session = SessionFile::load(&latest_path)?;
    Ok(Some(session))
}

pub fn load_session_by_id(session_id: &str) -> Result<Option<SessionFile>> {
    let path = sessions_dir().join(format!("{}.json", session_id));
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(SessionFile::load(&path)?))
}

pub fn list_recent_sessions(limit: usize) -> Result<Vec<SessionSummary>> {
    Ok(list_recent_sessions_report(limit)?.0)
}

pub fn list_recent_sessions_report(limit: usize) -> Result<(Vec<SessionSummary>, usize)> {
    list_recent_sessions_in_dir(&sessions_dir(), limit)
}

fn list_recent_sessions_in_dir(dir: &Path, limit: usize) -> Result<(Vec<SessionSummary>, usize)> {
    if limit == 0 || !dir.exists() {
        return Ok((Vec::new(), 0));
    }

    let mut summaries = Vec::new();
    let mut skipped = 0;
    for entry in std::fs::read_dir(dir).context("failed to read sessions directory")? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().map(|ext| ext == "json").unwrap_or(false) {
            if let Ok(session) = SessionFile::load(&path) {
                summaries.push(session.summary());
            } else {
                skipped += 1;
            }
        }
    }

    summaries.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    summaries.truncate(limit);
    Ok((summaries, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::message::{ContentBlock, Message, Usage};

    #[test]
    fn test_session_file_new() {
        let session = SessionFile::new("claude-test", "opusplan", Path::new("/tmp/test"));
        assert_eq!(session.model, "claude-test");
        assert_eq!(session.model_setting, "opusplan");
        assert_eq!(session.cwd, "/tmp/test");
        assert!(session.messages.is_empty());
        assert!(!session.id.is_empty());
        assert!(!session.created_at.is_empty());
    }

    #[test]
    fn test_restore_app_state_preserves_session_id() {
        let mut state = AppState::new(PathBuf::from("/tmp/test"));
        let mut session = SessionFile::new("claude-test", "opusplan", Path::new("/tmp/test"));
        session.id = "20260428_123456".into();

        restore_app_state_from_session(&mut state, &session);

        assert_eq!(state.session.id, "20260428_123456");
    }

    #[test]
    fn test_session_file_serde_roundtrip() {
        let mut session = SessionFile::new("claude-test", "haiku", Path::new("/tmp"));
        session.messages.push(Message::user("hello"));
        session
            .messages
            .push(Message::assistant(vec![ContentBlock::text("hi there")]));

        let json = serde_json::to_string(&session).unwrap();
        let parsed: SessionFile = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.model, "claude-test");
        assert_eq!(parsed.model_setting, "haiku");
        assert_eq!(parsed.messages.len(), 2);
    }

    #[test]
    fn test_session_summary_extracts_first_user_message() {
        let mut session = SessionFile::new("claude-test", "haiku", Path::new("/workspace"));
        session.id = "20260426_120000".into();
        session
            .messages
            .push(Message::assistant(vec![ContentBlock::text("ready")]));
        session.messages.push(Message::user(
            "please summarize this session with a compact title",
        ));

        let summary = session.summary();

        assert_eq!(summary.id, "20260426_120000");
        assert_eq!(summary.model_setting, "haiku");
        assert_eq!(summary.cwd, "/workspace");
        assert_eq!(summary.message_count, 2);
        assert_eq!(
            summary.first_user_summary,
            "please summarize this session with a compact title"
        );
    }

    #[test]
    fn test_session_summary_truncates_long_first_user_message() {
        let mut session = SessionFile::new("claude-test", "haiku", Path::new("/workspace"));
        session.messages.push(Message::user("a".repeat(120)));

        let summary = session.summary();

        assert_eq!(summary.first_user_summary.chars().count(), 83);
        assert!(summary.first_user_summary.ends_with("..."));
    }

    #[test]
    fn test_session_file_roundtrip_preserves_assistant_message_usage() {
        let mut session = SessionFile::new("claude-sonnet-4-6", "opusplan", Path::new("/tmp"));
        session.messages.push(Message::assistant_with_usage(
            vec![ContentBlock::text("large assistant turn")],
            Usage {
                input_tokens: 150_000,
                output_tokens: 40_000,
                cache_creation_input_tokens: 10_001,
                cache_read_input_tokens: 0,
            },
        ));

        let json = serde_json::to_string(&session).unwrap();
        let parsed: SessionFile = serde_json::from_str(&json).unwrap();

        let usage = parsed.messages[0]
            .usage
            .as_ref()
            .expect("usage should persist");
        assert_eq!(usage.input_tokens, 150_000);
        assert_eq!(usage.output_tokens, 40_000);
        assert_eq!(usage.cache_creation_input_tokens, 10_001);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    #[test]
    fn test_session_save_and_load() {
        // Use a temp directory to avoid polluting real sessions
        let temp_dir = std::env::temp_dir().join(format!("session-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut session = SessionFile::new("claude-test", "best", Path::new("/tmp"));
        session.messages.push(Message::user("test message"));

        // Override the session path for testing
        let path = temp_dir.join(format!("{}.json", session.id));
        session.updated_at = chrono::Local::now().to_rfc3339();
        let json = serde_json::to_string_pretty(&session).unwrap();
        std::fs::write(&path, json).unwrap();

        let loaded = SessionFile::load(&path).unwrap();
        assert_eq!(loaded.model, "claude-test");
        assert_eq!(loaded.model_setting, "best");
        assert_eq!(loaded.messages.len(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_load_backfills_missing_model_setting() {
        let temp_dir =
            std::env::temp_dir().join(format!("session-backfill-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let path = temp_dir.join("legacy.json");
        std::fs::write(
            &path,
            r#"{
  "id": "20260416_120000",
  "model": "claude-opus-4-6[1m]",
  "cwd": "/tmp",
  "created_at": "2026-04-16T12:00:00+08:00",
  "updated_at": "2026-04-16T12:00:00+08:00",
  "messages": []
}"#,
        )
        .unwrap();

        let loaded = SessionFile::load(&path).unwrap();
        assert_eq!(loaded.model_setting, "claude-opus-4-6[1m]");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_recent_sessions_sorts_skips_corrupt_and_limits() {
        let temp_dir = std::env::temp_dir().join(format!(
            "session-list-test-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut older = SessionFile::new("claude-test", "haiku", Path::new("/tmp/a"));
        older.id = "20260426_100000".into();
        older.updated_at = "2026-04-26T10:00:00+08:00".into();
        older.messages.push(Message::user("older"));
        std::fs::write(
            temp_dir.join("older.json"),
            serde_json::to_string_pretty(&older).unwrap(),
        )
        .unwrap();

        let mut newer = SessionFile::new("claude-test", "sonnet", Path::new("/tmp/b"));
        newer.id = "20260426_110000".into();
        newer.updated_at = "2026-04-26T11:00:00+08:00".into();
        newer.messages.push(Message::user("newer"));
        std::fs::write(
            temp_dir.join("newer.json"),
            serde_json::to_string_pretty(&newer).unwrap(),
        )
        .unwrap();
        std::fs::write(temp_dir.join("broken.json"), "{not json").unwrap();

        let (summaries, skipped) = list_recent_sessions_in_dir(&temp_dir, 1).unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(skipped, 1);
        assert_eq!(summaries[0].id, "20260426_110000");
        assert_eq!(summaries[0].first_user_summary, "newer");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_restore_app_state_from_session() {
        let mut state = AppState::new(PathBuf::from("/workspace"));
        state.session.model_setting = "old".into();
        state.permission_mode = PermissionMode::Plan;

        let mut session = SessionFile::new("claude-sonnet-4-6", "sonnet", Path::new("/tmp"));
        session.messages.push(Message::user("restore me"));
        session.total_usage = Some(Usage {
            input_tokens: 10,
            output_tokens: 5,
            cache_creation_input_tokens: 1,
            cache_read_input_tokens: 2,
        });
        session.permission_mode = PermissionMode::AcceptEdits;

        restore_app_state_from_session(&mut state, &session);

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.session.model_setting, "sonnet");
        assert_eq!(state.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(state.total_usage.input_tokens, 10);
        assert_eq!(state.total_usage.cache_read_input_tokens, 2);
        assert_eq!(state.session.model, "claude-sonnet-4-6");
    }

    #[test]
    fn test_sessions_dir() {
        let dir = sessions_dir();
        assert!(dir.to_string_lossy().contains("rust-claude-code"));
        assert!(dir.to_string_lossy().contains("sessions"));
    }

    #[test]
    fn test_load_latest_from_empty_dir() {
        // Should return None when sessions dir doesn't exist or is empty
        // This test is safe since it only reads
        let result = load_latest_session();
        // Don't assert specific outcome — it depends on whether sessions exist
        assert!(result.is_ok());
    }
}
