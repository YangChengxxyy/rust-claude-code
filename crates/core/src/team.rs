//! Local team metadata + mailbox storage (iteration 49 skeleton).
//!
//! This is the minimal, **local-only** foundation for the Team tool family
//! (`TeamCreate` / `TeamDelete` / `SendMessage`). It deliberately does *not*
//! spawn real teammate processes, talk to tmux/iTerm, or touch any remote
//! queue — it only persists team config and per-member mailbox files under a
//! local config directory, so `SendMessage` has a runnable "write to a
//! member's mailbox" semantic today.
//!
//! Path convention: one subdirectory per team under `<root>` (default root
//! `~/.config/rust-claude-code/teams`):
//! - `<root>/<sanitized-team>/team.json` — the [`Team`] metadata.
//! - `<root>/<sanitized-team>/mailboxes/<sanitized-member>.json` — that
//!   member's mailbox, a JSON array of [`MailboxMessage`] appended in order.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// On-disk schema version for a team config file.
const TEAM_SCHEMA_VERSION: u32 = 1;

/// A local team's metadata.
///
/// `members` are member names (the mailbox key). `agent_type` is the team-wide
/// default agent type, and `task_list` optionally binds the team to a
/// task-list scope (e.g. a session id) managed by [`crate::task_list`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    #[serde(default)]
    pub members: Vec<String>,
    /// Team-wide default agent type, if specified.
    #[serde(default)]
    pub agent_type: Option<String>,
    /// Bound task-list scope (session/team id), if any.
    #[serde(default)]
    pub task_list: Option<String>,
}

impl Team {
    /// Create a team with empty optionals and no members.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            members: Vec::new(),
            agent_type: None,
            task_list: None,
        }
    }

    /// True if `member` is listed in this team.
    pub fn has_member(&self, member: &str) -> bool {
        self.members.iter().any(|m| m == member)
    }
}

/// One message in a member's mailbox.
///
/// `seq` is assigned by [`TeamStore::append_message`] in append order
/// (1-based) and is deterministic across runs — no wall-clock timestamp is
/// stored, which keeps mailbox files reproducible for tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxMessage {
    pub seq: u64,
    pub from: String,
    pub content: String,
}

/// Wrapper written to `team.json` so future schema changes can be detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TeamConfig {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(flatten)]
    team: Team,
}

fn default_schema_version() -> u32 {
    TEAM_SCHEMA_VERSION
}

/// Errors raised by [`TeamStore`].
#[derive(Debug, thiserror::Error)]
pub enum TeamStoreError {
    #[error("HOME environment variable not set")]
    NoHomeDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Persists local teams to disk: one subdirectory per team.
///
/// The default root is `~/.config/rust-claude-code/teams` (see
/// [`TeamStore::default_root`]). A missing team directory is treated as
/// "team does not exist", not an error.
pub struct TeamStore {
    root: PathBuf,
}

impl TeamStore {
    /// Create a store rooted at `root`. Directories are created lazily on the
    /// first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default root: `$HOME/.config/rust-claude-code/teams`.
    pub fn default_root() -> Result<PathBuf, TeamStoreError> {
        let home = std::env::var("HOME").map_err(|_| TeamStoreError::NoHomeDir)?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("rust-claude-code")
            .join("teams"))
    }

    /// A store at the default root (see [`TeamStore::default_root`]).
    pub fn default_store() -> Result<Self, TeamStoreError> {
        Ok(Self::new(Self::default_root()?))
    }

    /// `<root>/<sanitized-name>`.
    fn team_dir(&self, name: &str) -> PathBuf {
        self.root.join(sanitize_name(name))
    }

    fn config_path(&self, name: &str) -> PathBuf {
        self.team_dir(name).join("team.json")
    }

    fn mailbox_path(&self, name: &str, member: &str) -> PathBuf {
        self.team_dir(name)
            .join("mailboxes")
            .join(format!("{}.json", sanitize_name(member)))
    }

    /// Whether a team config exists on disk.
    pub fn exists(&self, name: &str) -> bool {
        self.config_path(name).exists()
    }

    /// Load a team by name. Returns `Ok(None)` if it does not exist.
    pub fn load(&self, name: &str) -> Result<Option<Team>, TeamStoreError> {
        let path = self.config_path(name);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let config: TeamConfig = serde_json::from_slice(&bytes)?;
                Ok(Some(config.team))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Persist `team` to `<root>/<team>/team.json`, creating directories as
    /// needed. Overwrites any existing config for the same name.
    pub fn save(&self, team: &Team) -> Result<(), TeamStoreError> {
        let path = self.config_path(&team.name);
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir)?;
        let config = TeamConfig {
            schema_version: TEAM_SCHEMA_VERSION,
            team: team.clone(),
        };
        let json = serde_json::to_string_pretty(&config)?;
        // Temp-file + rename so a crash mid-write never leaves a truncated file.
        let tmp = dir.join(format!(
            ".{}.tmp",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("team")
        ));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Remove a team's entire directory (config + all mailboxes). Returns
    /// `Ok(true)` if it existed, `Ok(false)` if the team was already gone.
    pub fn delete(&self, name: &str) -> Result<bool, TeamStoreError> {
        let dir = self.team_dir(name);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    /// Read a member's mailbox (append order). An empty vec is returned when
    /// the mailbox does not exist yet.
    pub fn read_mailbox(
        &self,
        name: &str,
        member: &str,
    ) -> Result<Vec<MailboxMessage>, TeamStoreError> {
        let path = self.mailbox_path(name, member);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
    }

    /// Append `from`/`content` to `member`'s mailbox within team `name`,
    /// assigning the next 1-based `seq`. Returns the stored message.
    pub fn append_message(
        &self,
        name: &str,
        member: &str,
        from: &str,
        content: &str,
    ) -> Result<MailboxMessage, TeamStoreError> {
        let mut mailbox = self.read_mailbox(name, member)?;
        let seq = mailbox.len() as u64 + 1;
        let message = MailboxMessage {
            seq,
            from: from.to_string(),
            content: content.to_string(),
        };
        mailbox.push(message.clone());
        let path = self.mailbox_path(name, member);
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_string_pretty(&mailbox)?;
        let tmp = dir.join(format!(
            ".{}.tmp",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("mailbox")
        ));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(message)
    }
}

/// Reduce a team/member name to a single filename-safe path component,
/// rejecting path traversal. Anything outside `[A-Za-z0-9._-]` becomes `_`.
fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Guard against empty / dot-only names mapping to hidden or empty paths.
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        let unique = format!(
            "team-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ---- Team model ----

    #[test]
    fn new_team_has_empty_optionals() {
        let t = Team::new("alpha");
        assert_eq!(t.name, "alpha");
        assert!(t.members.is_empty());
        assert!(t.agent_type.is_none());
        assert!(t.task_list.is_none());
        assert!(!t.has_member("anyone"));
    }

    // ---- TeamStore save / load / exists / delete ----

    #[test]
    fn load_missing_team_is_none() {
        let store = TeamStore::new(temp_root());
        assert!(store.load("ghost").unwrap().is_none());
        assert!(!store.exists("ghost"));
    }

    #[test]
    fn save_then_load_round_trips() {
        let store = TeamStore::new(temp_root());
        let mut team = Team::new("alpha");
        team.members = vec!["worker-1".into(), "worker-2".into()];
        team.agent_type = Some("general-purpose".into());
        team.task_list = Some("session-42".into());
        store.save(&team).unwrap();

        assert!(store.exists("alpha"));
        let back = store.load("alpha").unwrap().unwrap();
        assert_eq!(back, team);
        assert!(back.has_member("worker-1"));
    }

    #[test]
    fn save_creates_team_directory_and_config_file() {
        let root = temp_root();
        let store = TeamStore::new(&root);
        store.save(&Team::new("alpha")).unwrap();
        assert!(root.join("alpha").is_dir());
        assert!(root.join("alpha").join("team.json").exists());
    }

    #[test]
    fn delete_removes_team_directory_and_reports_existence() {
        let store = TeamStore::new(temp_root());
        store.save(&Team::new("alpha")).unwrap();
        assert!(store.delete("alpha").unwrap());
        assert!(!store.exists("alpha"));
        // Deleting again reports that it was already gone.
        assert!(!store.delete("alpha").unwrap());
    }

    #[test]
    fn teams_isolate_by_name() {
        let store = TeamStore::new(temp_root());
        let mut a = Team::new("alpha");
        a.members = vec!["a-1".into()];
        let mut b = Team::new("beta");
        b.members = vec!["b-1".into()];
        store.save(&a).unwrap();
        store.save(&b).unwrap();
        assert_eq!(
            store.load("alpha").unwrap().unwrap().members,
            vec!["a-1".to_string()]
        );
        assert_eq!(
            store.load("beta").unwrap().unwrap().members,
            vec!["b-1".to_string()]
        );
    }

    // ---- mailbox ----

    #[test]
    fn append_message_assigns_sequential_seq_and_persists() {
        let store = TeamStore::new(temp_root());
        store.save(&Team::new("alpha")).unwrap();

        let m1 = store.append_message("alpha", "worker-1", "lead", "do x").unwrap();
        let m2 = store
            .append_message("alpha", "worker-1", "lead", "then y")
            .unwrap();
        assert_eq!(m1.seq, 1);
        assert_eq!(m2.seq, 2);

        let mailbox = store.read_mailbox("alpha", "worker-1").unwrap();
        assert_eq!(mailbox.len(), 2);
        assert_eq!(mailbox[0].content, "do x");
        assert_eq!(mailbox[1].from, "lead");
    }

    #[test]
    fn mailboxes_isolate_by_member() {
        let store = TeamStore::new(temp_root());
        store.save(&Team::new("alpha")).unwrap();
        store.append_message("alpha", "w1", "lead", "for w1").unwrap();
        store.append_message("alpha", "w2", "lead", "for w2").unwrap();
        assert_eq!(store.read_mailbox("alpha", "w1").unwrap().len(), 1);
        assert_eq!(store.read_mailbox("alpha", "w2").unwrap().len(), 1);
        // Each member's seq is independent.
        assert_eq!(store.read_mailbox("alpha", "w2").unwrap()[0].seq, 1);
    }

    #[test]
    fn delete_clears_member_mailboxes_too() {
        let store = TeamStore::new(temp_root());
        store.save(&Team::new("alpha")).unwrap();
        store.append_message("alpha", "w1", "lead", "hi").unwrap();
        store.delete("alpha").unwrap();
        // After delete, the mailbox file is gone — read yields empty.
        assert!(store.read_mailbox("alpha", "w1").unwrap().is_empty());
    }

    // ---- sanitization ----

    #[test]
    fn sanitize_name_rejects_traversal_and_special_chars() {
        assert_eq!(sanitize_name("alpha"), "alpha");
        assert_eq!(sanitize_name("team/a"), "team_a");
        assert_eq!(sanitize_name(".."), "default");
        assert_eq!(sanitize_name("with space"), "with_space");
        assert_eq!(sanitize_name(""), "default");
        assert!(!sanitize_name("../etc/passwd").contains('/'));
    }

    #[test]
    fn default_root_points_under_config_dir_when_home_set() {
        if std::env::var("HOME").is_ok() {
            let root = TeamStore::default_root().unwrap();
            assert!(root.ends_with("teams"));
            assert!(root.to_string_lossy().contains(".config/rust-claude-code"));
        }
    }
}
