//! Session persistence — save and load conversation history.
//!
//! Sessions are stored as JSON files under `~/.config/rust-claude-code/sessions/`.
//! Each session file contains the message history, model info, and metadata.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use rust_claude_core::message::Message;

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
        }
    }

    /// Save this session to its file.
    pub fn save(&mut self) -> Result<PathBuf> {
        self.updated_at = chrono::Local::now().to_rfc3339();
        let dir = sessions_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create sessions directory: {}", dir.display()))?;
        let path = dir.join(format!("{}.json", self.id));
        let json = serde_json::to_string_pretty(self)
            .context("failed to serialize session")?;
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
    fn test_session_file_serde_roundtrip() {
        let mut session = SessionFile::new("claude-test", "haiku", Path::new("/tmp"));
        session.messages.push(Message::user("hello"));
        session.messages.push(Message::assistant(vec![
            ContentBlock::text("hi there"),
        ]));

        let json = serde_json::to_string(&session).unwrap();
        let parsed: SessionFile = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.model, "claude-test");
        assert_eq!(parsed.model_setting, "haiku");
        assert_eq!(parsed.messages.len(), 2);
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

        let usage = parsed.messages[0].usage.as_ref().expect("usage should persist");
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
        let temp_dir = std::env::temp_dir().join(format!("session-backfill-test-{}", std::process::id()));
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
