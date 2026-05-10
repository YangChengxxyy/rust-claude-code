//! Session persistence — save and load conversation history.
//!
//! Sessions are stored as JSONL (append-only) or legacy JSON files under
//! `~/.config/rust-claude-code/sessions/`.
//!
//! New sessions use `.jsonl` format with incremental writes.
//! Old `.json` sessions remain loadable for backward compatibility.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use rust_claude_core::message::{ContentBlock, Message, Role, Usage};
use rust_claude_core::model::get_runtime_main_loop_model;
use rust_claude_core::permission::{PermissionMode, PermissionRule};
use rust_claude_core::session::{SessionEvent, SessionSummary};
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

    /// Load a session from a file path, auto-detecting format by extension.
    ///
    /// - `.jsonl` → JSONL event log parser
    /// - `.json` (or other) → legacy JSON snapshot parser
    pub fn load(path: &Path) -> Result<Self> {
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            load_from_jsonl(path)
        } else {
            Self::load_from_json(path)
        }
    }

    /// Load a session from a legacy JSON snapshot file.
    fn load_from_json(path: &Path) -> Result<Self> {
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
            interrupted: false,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionWriter — incremental JSONL append writer
// ---------------------------------------------------------------------------

/// Incrementally writes session events to a `.jsonl` file.
///
/// Each call to `append_message` or `append_event` serializes one JSON line
/// and flushes to disk, so that crash recovery can replay up to the last
/// successfully written event.
pub struct SessionWriter {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl SessionWriter {
    /// Create a new JSONL session file and write the `Header` event.
    pub fn new(id: &str, model: &str, model_setting: &str, cwd: &Path) -> Result<Self> {
        let dir = sessions_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create sessions directory: {}", dir.display()))?;
        let path = dir.join(format!("{}.jsonl", id));
        let file = File::create(&path)
            .with_context(|| format!("failed to create session file: {}", path.display()))?;
        let mut writer = BufWriter::new(file);

        let header = SessionEvent::Header {
            id: id.to_string(),
            model: model.to_string(),
            model_setting: model_setting.to_string(),
            cwd: cwd.display().to_string(),
            created_at: chrono::Local::now().to_rfc3339(),
        };
        let line = serde_json::to_string(&header).context("failed to serialize header event")?;
        writeln!(writer, "{}", line).context("failed to write header event")?;
        writer.flush().context("failed to flush header")?;

        Ok(SessionWriter { writer, path })
    }

    /// Open an existing JSONL session file in append mode (for crash recovery resume).
    pub fn open_append(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .append(true)
            .open(path)
            .with_context(|| format!("failed to open session file for append: {}", path.display()))?;
        let writer = BufWriter::new(file);
        Ok(SessionWriter {
            writer,
            path: path.to_path_buf(),
        })
    }

    /// Append a message event. The event type is chosen based on the message role.
    pub fn append_message(&mut self, msg: &Message) -> Result<()> {
        let event = match msg.role {
            Role::User => SessionEvent::UserMessage {
                message: msg.clone(),
            },
            Role::Assistant => SessionEvent::AssistantMessage {
                message: msg.clone(),
            },
        };
        self.append_event(&event)
    }

    /// Append an arbitrary session event.
    pub fn append_event(&mut self, event: &SessionEvent) -> Result<()> {
        let line =
            serde_json::to_string(event).context("failed to serialize session event")?;
        writeln!(self.writer, "{}", line).context("failed to write session event")?;
        self.writer.flush().context("failed to flush session event")?;
        Ok(())
    }

    /// Write a `SessionEnd` event and flush. After this, the writer should not be used.
    pub fn finish(&mut self) -> Result<()> {
        let event = SessionEvent::SessionEnd {
            updated_at: chrono::Local::now().to_rfc3339(),
        };
        self.append_event(&event)
    }

    /// Return the path of the JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl rust_claude_sdk::output::SessionPersistence for SessionWriter {
    fn persist_message(
        &mut self,
        msg: &Message,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.append_message(msg).map_err(|e| e.to_string().into())
    }

    fn persist_event(
        &mut self,
        event: &SessionEvent,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.append_event(event).map_err(|e| e.to_string().into())
    }
}

// ---------------------------------------------------------------------------
// SessionReader — JSONL loader
// ---------------------------------------------------------------------------

/// Load a session from a JSONL file, reconstructing `SessionFile` from events.
///
/// Invalid lines (e.g. truncated by crash) are skipped with a warning log.
pub fn load_from_jsonl(path: &Path) -> Result<SessionFile> {
    let file = File::open(path)
        .with_context(|| format!("failed to open JSONL session file: {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut session = SessionFile {
        id: String::new(),
        model: String::new(),
        model_setting: String::new(),
        cwd: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
        messages: Vec::new(),
        total_usage: None,
        permission_mode: PermissionMode::Default,
        always_allow_rules: Vec::new(),
        always_deny_rules: Vec::new(),
    };
    let mut has_session_end = false;

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "warning: failed to read line {} of {}: {}",
                    line_num + 1,
                    path.display(),
                    e
                );
                continue;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let event: SessionEvent = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "warning: skipping invalid JSONL line {} in {}: {}",
                    line_num + 1,
                    path.display(),
                    e
                );
                continue;
            }
        };

        match event {
            SessionEvent::Header {
                id,
                model,
                model_setting,
                cwd,
                created_at,
            } => {
                session.id = id;
                session.model = model;
                session.model_setting = model_setting;
                session.cwd = cwd;
                session.created_at = created_at.clone();
                session.updated_at = created_at;
            }
            SessionEvent::UserMessage { message } | SessionEvent::AssistantMessage { message } => {
                session.messages.push(message);
            }
            SessionEvent::CompactBoundary { summary } => {
                // Replace all previous messages with a single summary message
                session.messages.clear();
                session
                    .messages
                    .push(Message::user(format!("[Compaction Summary] {}", summary)));
            }
            SessionEvent::UsageUpdate { usage } => {
                session.total_usage = Some(usage);
            }
            SessionEvent::PermissionChange {
                mode,
                allow_rules,
                deny_rules,
            } => {
                session.permission_mode = mode;
                session.always_allow_rules = allow_rules;
                session.always_deny_rules = deny_rules;
            }
            SessionEvent::SessionEnd { updated_at } => {
                session.updated_at = updated_at;
                has_session_end = true;
            }
        }
    }

    // If no SessionEnd was found, the session was interrupted — set updated_at
    // to created_at (or leave as-is from Header).
    let _ = has_session_end; // used by callers via is_interrupted_jsonl

    if session.model_setting.is_empty() && !session.model.is_empty() {
        session.model_setting = session.model.clone();
    }

    Ok(session)
}

/// Check whether a `.jsonl` session file is interrupted (missing `SessionEnd`).
///
/// This reads the file and checks if any line is a `SessionEnd` event.
/// For efficiency on large files, it reads from the end looking for the marker.
pub fn is_interrupted_jsonl(path: &Path) -> bool {
    // Quick check: read the last few lines looking for session_end
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    // Check last non-empty lines for session_end type
    for line in content.lines().rev().take(5) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains("\"type\":\"session_end\"") {
            return false;
        }
        // Once we find a non-empty, non-session_end line, stop
        break;
    }
    // If we have at least one line (header), it's interrupted
    content.lines().any(|l| !l.trim().is_empty())
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
///
/// Scans for both `.json` and `.jsonl` files. If an interrupted `.jsonl`
/// session exists that is more recent, it is preferred (for crash recovery).
pub fn load_latest_session() -> Result<Option<SessionFile>> {
    load_latest_session_in_dir(&sessions_dir())
}

fn load_latest_session_in_dir(dir: &Path) -> Result<Option<SessionFile>> {
    if !dir.exists() {
        return Ok(None);
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .context("failed to read sessions directory")?
        .filter_map(|e| e.ok())
        .filter(|e| is_session_file(&e.path()))
        .collect();

    if entries.is_empty() {
        return Ok(None);
    }

    // Sort by filename in descending order (timestamp-based)
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    // Prefer interrupted .jsonl sessions (crash recovery)
    for entry in &entries {
        let path = entry.path();
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) && is_interrupted_jsonl(&path)
        {
            if let Ok(session) = SessionFile::load(&path) {
                return Ok(Some(session));
            }
        }
    }

    // Fall back to most recent session (any format).
    // Deduplicate: if both {id}.json and {id}.jsonl exist, prefer .jsonl
    let latest_path = dedup_session_path(&entries[0].path());
    let session = SessionFile::load(&latest_path)?;
    Ok(Some(session))
}

/// If both `{id}.json` and `{id}.jsonl` exist, prefer `.jsonl`.
fn dedup_session_path(path: &Path) -> PathBuf {
    if path.extension().map(|e| e == "json").unwrap_or(false) {
        let jsonl_path = path.with_extension("jsonl");
        if jsonl_path.exists() {
            return jsonl_path;
        }
    }
    path.to_path_buf()
}

/// Returns `true` if the path has a `.json` or `.jsonl` extension.
fn is_session_file(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext == "json" || ext == "jsonl")
        .unwrap_or(false)
}

pub fn load_session_by_id(session_id: &str) -> Result<Option<SessionFile>> {
    let dir = sessions_dir();
    // Prefer .jsonl over .json
    let jsonl_path = dir.join(format!("{}.jsonl", session_id));
    if jsonl_path.exists() {
        return Ok(Some(SessionFile::load(&jsonl_path)?));
    }
    let json_path = dir.join(format!("{}.json", session_id));
    if json_path.exists() {
        return Ok(Some(SessionFile::load(&json_path)?));
    }
    Ok(None)
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

    // Collect all session files, deduplicating ids (prefer .jsonl over .json)
    let mut seen_ids = std::collections::HashSet::new();
    let mut session_paths: Vec<PathBuf> = Vec::new();

    // First pass: collect .jsonl files
    for entry in std::fs::read_dir(dir).context("failed to read sessions directory")? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                seen_ids.insert(stem.to_string());
                session_paths.push(path);
            }
        }
    }

    // Second pass: collect .json files not already covered by .jsonl
    for entry in std::fs::read_dir(dir).context("failed to read sessions directory")? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().map(|ext| ext == "json").unwrap_or(false) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !seen_ids.contains(stem) {
                    session_paths.push(path);
                }
            }
        }
    }

    let mut summaries = Vec::new();
    let mut skipped = 0;
    for path in &session_paths {
        match SessionFile::load(path) {
            Ok(session) => {
                let mut summary = session.summary();
                // Detect interrupted .jsonl sessions
                if path.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
                    summary.interrupted = is_interrupted_jsonl(path);
                }
                summaries.push(summary);
            }
            Err(_) => {
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

    // --- SessionWriter tests ---

    fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "session-{}-{}-{}",
            suffix,
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_session_writer_creates_jsonl_with_header() {
        let temp_dir = make_temp_dir("writer-header");
        // Temporarily override sessions dir by creating writer manually
        let path = temp_dir.join("test_session.jsonl");
        let file = File::create(&path).unwrap();
        let mut writer = BufWriter::new(file);

        let header = SessionEvent::Header {
            id: "20260504_143022".to_string(),
            model: "claude-test".to_string(),
            model_setting: "sonnet".to_string(),
            cwd: "/tmp".to_string(),
            created_at: "2026-05-04T14:30:22+08:00".to_string(),
        };
        let line = serde_json::to_string(&header).unwrap();
        writeln!(writer, "{}", line).unwrap();
        writer.flush().unwrap();
        drop(writer);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\"type\":\"header\""));
        assert!(lines[0].contains("\"id\":\"20260504_143022\""));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_writer_append_messages_and_finish() {
        let temp_dir = make_temp_dir("writer-append");
        let path = temp_dir.join("session.jsonl");

        // Write header
        let file = File::create(&path).unwrap();
        let mut bw = BufWriter::new(file);
        let header = SessionEvent::Header {
            id: "test".to_string(),
            model: "m".to_string(),
            model_setting: "m".to_string(),
            cwd: "/tmp".to_string(),
            created_at: "t".to_string(),
        };
        writeln!(bw, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        bw.flush().unwrap();
        drop(bw);

        // Open in append mode and write messages
        let mut sw = SessionWriter::open_append(&path).unwrap();
        sw.append_message(&Message::user("hello")).unwrap();
        sw.append_message(&Message::assistant(vec![ContentBlock::text("hi")]))
            .unwrap();
        sw.finish().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4); // header + user + assistant + session_end
        assert!(lines[1].contains("\"type\":\"user_message\""));
        assert!(lines[2].contains("\"type\":\"assistant_message\""));
        assert!(lines[3].contains("\"type\":\"session_end\""));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_writer_append_event() {
        let temp_dir = make_temp_dir("writer-event");
        let path = temp_dir.join("session.jsonl");

        let file = File::create(&path).unwrap();
        let mut bw = BufWriter::new(file);
        let header = SessionEvent::Header {
            id: "test".to_string(),
            model: "m".to_string(),
            model_setting: "m".to_string(),
            cwd: "/".to_string(),
            created_at: "t".to_string(),
        };
        writeln!(bw, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        bw.flush().unwrap();
        drop(bw);

        let mut sw = SessionWriter::open_append(&path).unwrap();
        sw.append_event(&SessionEvent::CompactBoundary {
            summary: "test summary".to_string(),
        })
        .unwrap();
        sw.append_event(&SessionEvent::UsageUpdate {
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        })
        .unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[1].contains("\"type\":\"compact_boundary\""));
        assert!(lines[2].contains("\"type\":\"usage_update\""));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // --- SessionReader (load_from_jsonl) tests ---

    fn write_jsonl_file(path: &Path, events: &[SessionEvent]) {
        let mut file = File::create(path).unwrap();
        for event in events {
            writeln!(file, "{}", serde_json::to_string(event).unwrap()).unwrap();
        }
    }

    #[test]
    fn test_load_from_jsonl_normal() {
        let temp_dir = make_temp_dir("reader-normal");
        let path = temp_dir.join("session.jsonl");

        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "20260504_100000".to_string(),
                    model: "claude-test".to_string(),
                    model_setting: "sonnet".to_string(),
                    cwd: "/workspace".to_string(),
                    created_at: "2026-05-04T10:00:00+08:00".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("hello"),
                },
                SessionEvent::AssistantMessage {
                    message: Message::assistant(vec![ContentBlock::text("hi")]),
                },
                SessionEvent::UsageUpdate {
                    usage: Usage {
                        input_tokens: 100,
                        output_tokens: 50,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                },
                SessionEvent::SessionEnd {
                    updated_at: "2026-05-04T10:05:00+08:00".to_string(),
                },
            ],
        );

        let session = load_from_jsonl(&path).unwrap();
        assert_eq!(session.id, "20260504_100000");
        assert_eq!(session.model, "claude-test");
        assert_eq!(session.model_setting, "sonnet");
        assert_eq!(session.cwd, "/workspace");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.total_usage.as_ref().unwrap().input_tokens, 100);
        assert_eq!(session.updated_at, "2026-05-04T10:05:00+08:00");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_from_jsonl_truncated_last_line() {
        let temp_dir = make_temp_dir("reader-truncated");
        let path = temp_dir.join("session.jsonl");

        // Write a valid header and message, then a truncated line
        let mut file = File::create(&path).unwrap();
        let header = SessionEvent::Header {
            id: "test".to_string(),
            model: "m".to_string(),
            model_setting: "m".to_string(),
            cwd: "/".to_string(),
            created_at: "t".to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        let msg_event = SessionEvent::UserMessage {
            message: Message::user("hello"),
        };
        writeln!(file, "{}", serde_json::to_string(&msg_event).unwrap()).unwrap();
        // Truncated line
        writeln!(file, "{{\"type\":\"assistant_message\",\"message\":{{\"role\":\"as").unwrap();
        drop(file);

        let session = load_from_jsonl(&path).unwrap();
        assert_eq!(session.id, "test");
        assert_eq!(session.messages.len(), 1); // only the user message
        assert_eq!(session.messages[0].role, Role::User);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_from_jsonl_compact_boundary() {
        let temp_dir = make_temp_dir("reader-compact");
        let path = temp_dir.join("session.jsonl");

        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "test".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "t".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("old message 1"),
                },
                SessionEvent::AssistantMessage {
                    message: Message::assistant(vec![ContentBlock::text("old reply")]),
                },
                SessionEvent::CompactBoundary {
                    summary: "User discussed file structure.".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("new message after compact"),
                },
                SessionEvent::SessionEnd {
                    updated_at: "t2".to_string(),
                },
            ],
        );

        let session = load_from_jsonl(&path).unwrap();
        // After CompactBoundary, old messages are replaced by summary + new message
        assert_eq!(session.messages.len(), 2);
        assert!(session.messages[0].content[0]
            .eq(&ContentBlock::text("[Compaction Summary] User discussed file structure.")));
        assert_eq!(session.messages[1].role, Role::User);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_from_jsonl_unknown_event_type_skipped() {
        let temp_dir = make_temp_dir("reader-unknown");
        let path = temp_dir.join("session.jsonl");

        let mut file = File::create(&path).unwrap();
        let header = SessionEvent::Header {
            id: "test".to_string(),
            model: "m".to_string(),
            model_setting: "m".to_string(),
            cwd: "/".to_string(),
            created_at: "t".to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        // Unknown event type
        writeln!(file, r#"{{"type":"future_event","data":"something"}}"#).unwrap();
        let msg = SessionEvent::UserMessage {
            message: Message::user("still here"),
        };
        writeln!(file, "{}", serde_json::to_string(&msg).unwrap()).unwrap();
        drop(file);

        let session = load_from_jsonl(&path).unwrap();
        assert_eq!(session.id, "test");
        assert_eq!(session.messages.len(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_from_jsonl_permission_change() {
        let temp_dir = make_temp_dir("reader-perm");
        let path = temp_dir.join("session.jsonl");

        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "test".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "t".to_string(),
                },
                SessionEvent::PermissionChange {
                    mode: PermissionMode::AcceptEdits,
                    allow_rules: vec![],
                    deny_rules: vec![],
                },
                SessionEvent::SessionEnd {
                    updated_at: "t2".to_string(),
                },
            ],
        );

        let session = load_from_jsonl(&path).unwrap();
        assert_eq!(session.permission_mode, PermissionMode::AcceptEdits);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // --- Auto-detection tests ---

    #[test]
    fn test_session_file_load_autodetects_jsonl() {
        let temp_dir = make_temp_dir("autodetect-jsonl");
        let path = temp_dir.join("session.jsonl");

        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "auto-test".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "t".to_string(),
                },
                SessionEvent::SessionEnd {
                    updated_at: "t2".to_string(),
                },
            ],
        );

        let session = SessionFile::load(&path).unwrap();
        assert_eq!(session.id, "auto-test");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_file_load_autodetects_json() {
        let temp_dir = make_temp_dir("autodetect-json");
        let path = temp_dir.join("session.json");

        let session = SessionFile::new("claude-test", "haiku", Path::new("/tmp"));
        std::fs::write(&path, serde_json::to_string_pretty(&session).unwrap()).unwrap();

        let loaded = SessionFile::load(&path).unwrap();
        assert_eq!(loaded.model, "claude-test");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // --- Crash recovery tests ---

    #[test]
    fn test_is_interrupted_jsonl_without_session_end() {
        let temp_dir = make_temp_dir("interrupted-true");
        let path = temp_dir.join("session.jsonl");

        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "test".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "t".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("hello"),
                },
            ],
        );

        assert!(is_interrupted_jsonl(&path));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_is_interrupted_jsonl_with_session_end() {
        let temp_dir = make_temp_dir("interrupted-false");
        let path = temp_dir.join("session.jsonl");

        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "test".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "t".to_string(),
                },
                SessionEvent::SessionEnd {
                    updated_at: "t2".to_string(),
                },
            ],
        );

        assert!(!is_interrupted_jsonl(&path));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_writer_open_append_continues_file() {
        let temp_dir = make_temp_dir("writer-resume");
        let path = temp_dir.join("session.jsonl");

        // Write header + one message (simulating interrupted session)
        write_jsonl_file(
            &path,
            &[
                SessionEvent::Header {
                    id: "resume-test".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "t".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("original"),
                },
            ],
        );

        assert!(is_interrupted_jsonl(&path));

        // Resume by appending
        let mut sw = SessionWriter::open_append(&path).unwrap();
        sw.append_message(&Message::user("resumed message")).unwrap();
        sw.finish().unwrap();

        // Verify full content
        let session = load_from_jsonl(&path).unwrap();
        assert_eq!(session.id, "resume-test");
        assert_eq!(session.messages.len(), 2);
        assert!(!is_interrupted_jsonl(&path));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // --- Mixed-format directory scanning tests ---

    #[test]
    fn test_list_sessions_includes_both_formats() {
        let temp_dir = make_temp_dir("list-mixed");

        // Create a .json session
        let mut json_session = SessionFile::new("claude-test", "haiku", Path::new("/tmp/a"));
        json_session.id = "20260426_100000".into();
        json_session.updated_at = "2026-04-26T10:00:00+08:00".into();
        json_session.messages.push(Message::user("json session"));
        std::fs::write(
            temp_dir.join("20260426_100000.json"),
            serde_json::to_string_pretty(&json_session).unwrap(),
        )
        .unwrap();

        // Create a .jsonl session
        write_jsonl_file(
            &temp_dir.join("20260426_110000.jsonl"),
            &[
                SessionEvent::Header {
                    id: "20260426_110000".to_string(),
                    model: "claude-test".to_string(),
                    model_setting: "sonnet".to_string(),
                    cwd: "/tmp/b".to_string(),
                    created_at: "2026-04-26T11:00:00+08:00".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("jsonl session"),
                },
                SessionEvent::SessionEnd {
                    updated_at: "2026-04-26T11:05:00+08:00".to_string(),
                },
            ],
        );

        let (summaries, skipped) = list_recent_sessions_in_dir(&temp_dir, 10).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(summaries.len(), 2);
        // Most recent first (jsonl session has later updated_at)
        assert_eq!(summaries[0].id, "20260426_110000");
        assert_eq!(summaries[1].id, "20260426_100000");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_sessions_deduplicates_jsonl_over_json() {
        let temp_dir = make_temp_dir("list-dedup");

        // Create both .json and .jsonl with the same id
        let mut json_session = SessionFile::new("claude-test", "haiku", Path::new("/tmp"));
        json_session.id = "20260426_100000".into();
        json_session.updated_at = "2026-04-26T10:00:00+08:00".into();
        json_session.messages.push(Message::user("old json"));
        std::fs::write(
            temp_dir.join("20260426_100000.json"),
            serde_json::to_string_pretty(&json_session).unwrap(),
        )
        .unwrap();

        write_jsonl_file(
            &temp_dir.join("20260426_100000.jsonl"),
            &[
                SessionEvent::Header {
                    id: "20260426_100000".to_string(),
                    model: "claude-test".to_string(),
                    model_setting: "sonnet".to_string(),
                    cwd: "/tmp".to_string(),
                    created_at: "2026-04-26T10:00:00+08:00".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("new jsonl"),
                },
                SessionEvent::SessionEnd {
                    updated_at: "2026-04-26T10:10:00+08:00".to_string(),
                },
            ],
        );

        let (summaries, _) = list_recent_sessions_in_dir(&temp_dir, 10).unwrap();
        // Only one entry (dedup)
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "20260426_100000");
        // The .jsonl version should be loaded (has "new jsonl" message)
        assert_eq!(summaries[0].first_user_summary, "new jsonl");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_sessions_detects_interrupted() {
        let temp_dir = make_temp_dir("list-interrupted");

        write_jsonl_file(
            &temp_dir.join("20260426_100000.jsonl"),
            &[
                SessionEvent::Header {
                    id: "20260426_100000".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "2026-04-26T10:00:00+08:00".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("crashed"),
                },
                // No SessionEnd — interrupted!
            ],
        );

        let (summaries, _) = list_recent_sessions_in_dir(&temp_dir, 10).unwrap();
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].interrupted);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_latest_prefers_interrupted_session() {
        let temp_dir = make_temp_dir("latest-interrupted");

        // Older completed session
        let mut completed = SessionFile::new("m", "m", Path::new("/"));
        completed.id = "20260426_100000".into();
        completed.updated_at = "2026-04-26T10:00:00+08:00".into();
        completed.messages.push(Message::user("completed"));
        std::fs::write(
            temp_dir.join("20260426_100000.json"),
            serde_json::to_string_pretty(&completed).unwrap(),
        )
        .unwrap();

        // Newer interrupted .jsonl session
        write_jsonl_file(
            &temp_dir.join("20260426_090000.jsonl"),
            &[
                SessionEvent::Header {
                    id: "20260426_090000".to_string(),
                    model: "m".to_string(),
                    model_setting: "m".to_string(),
                    cwd: "/".to_string(),
                    created_at: "2026-04-26T09:00:00+08:00".to_string(),
                },
                SessionEvent::UserMessage {
                    message: Message::user("interrupted"),
                },
                // No SessionEnd
            ],
        );

        let session = load_latest_session_in_dir(&temp_dir).unwrap().unwrap();
        // Should prefer the interrupted session
        assert_eq!(session.id, "20260426_090000");
        assert_eq!(session.messages.len(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
