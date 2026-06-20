//! Persisted task-list data model and storage.
//!
//! This is the foundation for the original-style Task tool family
//! (`TaskCreate` / `TaskGet` / `TaskList` / `TaskUpdate`) and future Team /
//! agent collaboration. It is deliberately **independent** of the in-memory
//! session todo list (`state::Task` / `AppState.tasks`): task-list entries are
//! richer (owner + dependency graph + arbitrary metadata) and are persisted per
//! scope (team or session) to disk so they survive across runs and can be
//! shared between agents.
//!
//! Scope isolation: each scope maps to one JSON file
//! `<root>/<sanitized-scope>.json` (default root `~/.config/rust-claude-code/tasks`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::state::TaskStatus;

/// Current on-disk schema version for [`TaskList`].
const TASK_LIST_SCHEMA_VERSION: u32 = 1;

/// A rich task record stored in a [`TaskList`].
///
/// Unlike the session todo [`crate::state::Task`], an entry carries an `owner`
/// (the agent that claimed it) and a dependency graph (`blocked_by` / `blocks`)
/// plus free-form `metadata`, which is what the multi-agent Task tool family and
/// Team coordination need.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskListEntry {
    /// Stable identifier within the scope. Numeric strings ("1", "2", ...) by
    /// convention — see [`TaskList::next_id`].
    pub id: String,
    pub subject: String,
    #[serde(default)]
    pub description: String,
    pub status: TaskStatus,
    /// Agent name that owns/claimed this task, if any.
    #[serde(default)]
    pub owner: Option<String>,
    /// IDs of tasks within the same scope that must finish before this one.
    #[serde(default)]
    pub blocked_by: Vec<String>,
    /// IDs of tasks within the same scope waiting on this one.
    #[serde(default)]
    pub blocks: Vec<String>,
    /// Free-form metadata (string keys, JSON values).
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskListEntry {
    /// Create a new pending entry with empty optional fields.
    pub fn new(id: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            subject: subject.into(),
            description: String::new(),
            status: TaskStatus::Pending,
            owner: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Patch applied by [`TaskList::update`].
///
/// Each field is `Option`; only `Some` fields are applied. For `owner`, the
/// outer `Option` means "is this field being updated" and the inner `Option`
/// means the new value — so `Some(None)` clears the owner while `Some(Some(x))`
/// sets it. `None` leaves the owner untouched.
#[derive(Debug, Clone, Default)]
pub struct TaskUpdate {
    pub subject: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub owner: Option<Option<String>>,
    pub blocked_by: Option<Vec<String>>,
    pub blocks: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// A collection of [`TaskListEntry`] values that serializes to one JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskList {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(default)]
    pub tasks: Vec<TaskListEntry>,
}

fn default_schema_version() -> u32 {
    TASK_LIST_SCHEMA_VERSION
}

impl TaskList {
    /// Create an empty task list at the current schema version.
    pub fn new() -> Self {
        Self {
            schema_version: TASK_LIST_SCHEMA_VERSION,
            tasks: Vec::new(),
        }
    }

    /// On-disk schema version this list was loaded/created with.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Look up an entry by id.
    pub fn get(&self, id: &str) -> Option<&TaskListEntry> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Look up an entry by id for mutation.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut TaskListEntry> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Entries sorted by id (numeric where possible, then lexicographic) so
    /// listing is stable across runs regardless of insertion order.
    pub fn list_sorted(&self) -> Vec<&TaskListEntry> {
        let mut refs: Vec<&TaskListEntry> = self.tasks.iter().collect();
        refs.sort_by(|a, b| sort_key(&a.id).cmp(&sort_key(&b.id)));
        refs
    }

    /// Insert or replace an entry by id. Returns the previous entry if one with
    /// the same id existed.
    pub fn upsert(&mut self, entry: TaskListEntry) -> Option<TaskListEntry> {
        if let Some(existing) = self.tasks.iter_mut().find(|t| t.id == entry.id) {
            let old = std::mem::replace(existing, entry);
            Some(old)
        } else {
            self.tasks.push(entry);
            None
        }
    }

    /// Remove an entry by id, returning it.
    pub fn remove(&mut self, id: &str) -> Option<TaskListEntry> {
        let pos = self.tasks.iter().position(|t| t.id == id)?;
        Some(self.tasks.remove(pos))
    }

    /// Apply a [`TaskUpdate`] patch to the entry with `id`. Returns a reference
    /// to the updated entry, or `None` if no entry has that id.
    pub fn update(&mut self, id: &str, patch: &TaskUpdate) -> Option<&TaskListEntry> {
        let entry = self.get_mut(id)?;
        if let Some(subject) = patch.subject.clone() {
            entry.subject = subject;
        }
        if let Some(description) = patch.description.clone() {
            entry.description = description;
        }
        if let Some(status) = patch.status {
            entry.status = status;
        }
        if let Some(owner) = patch.owner.clone() {
            entry.owner = owner;
        }
        if let Some(blocked_by) = patch.blocked_by.clone() {
            entry.blocked_by = blocked_by;
        }
        if let Some(blocks) = patch.blocks.clone() {
            entry.blocks = blocks;
        }
        if let Some(metadata) = patch.metadata.clone() {
            entry.metadata = metadata;
        }
        Some(entry)
    }

    /// Next sequential numeric id ("1", "2", ...) based on the highest existing
    /// numeric id. Non-numeric ids are ignored when computing the max.
    pub fn next_id(&self) -> String {
        let max = self
            .tasks
            .iter()
            .filter_map(|t| t.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        (max + 1).to_string()
    }
}

impl Default for TaskList {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort key: numeric ids sort by their integer value, non-numeric ids sort
/// after all numeric ones (by string).
fn sort_key(id: &str) -> (u8, u64, String) {
    match id.parse::<u64>() {
        Ok(n) => (0, n, String::new()),
        Err(_) => (1, u64::MAX, id.to_string()),
    }
}

/// Errors raised by [`TaskStore`].
#[derive(Debug, thiserror::Error)]
pub enum TaskStoreError {
    #[error("HOME environment variable not set")]
    NoHomeDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Persists [`TaskList`]s to disk, one JSON file per scope.
///
/// Path convention: `<root>/<sanitized-scope>.json`. The default root is
/// `~/.config/rust-claude-code/tasks` (see [`TaskStore::default_root`]). A
/// missing file is treated as an empty list, not an error.
pub struct TaskStore {
    root: PathBuf,
}

impl TaskStore {
    /// Create a store rooted at `root`. The directory is created lazily on the
    /// first [`TaskStore::save`].
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default root: `$HOME/.config/rust-claude-code/tasks`.
    pub fn default_root() -> Result<PathBuf, TaskStoreError> {
        let home = std::env::var("HOME").map_err(|_| TaskStoreError::NoHomeDir)?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("rust-claude-code")
            .join("tasks"))
    }

    /// A store at the default root (see [`TaskStore::default_root`]).
    pub fn default_store() -> Result<Self, TaskStoreError> {
        Ok(Self::new(Self::default_root()?))
    }

    fn scope_path(&self, scope: &str) -> PathBuf {
        self.root.join(format!("{}.json", sanitize_scope(scope)))
    }

    /// Load the task list for `scope`. A missing file yields an empty list.
    pub fn load(&self, scope: &str) -> Result<TaskList, TaskStoreError> {
        let path = self.scope_path(scope);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(TaskList::new()),
            Err(err) => Err(err.into()),
        }
    }

    /// Persist the task list for `scope`, creating the root directory if needed.
    pub fn save(&self, scope: &str, list: &TaskList) -> Result<(), TaskStoreError> {
        let path = self.scope_path(scope);
        std::fs::create_dir_all(&self.root)?;
        let json = serde_json::to_string_pretty(list)?;
        // Write to a temp file in the same directory, then rename, so a crash
        // mid-write never leaves a truncated task file.
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let tmp = dir.join(format!(
            ".{}.tmp",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("tasks")
        ));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// Reduce a scope (team/session id) to a single filename-safe path component,
/// rejecting path traversal. Anything outside `[A-Za-z0-9._-]` becomes `_`.
fn sanitize_scope(scope: &str) -> String {
    let cleaned: String = scope
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Guard against empty / dot-only names mapping to hidden or empty files.
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
    use crate::state::TaskStatus;
    use serde_json::json;

    fn entry(id: &str, subject: &str) -> TaskListEntry {
        TaskListEntry::new(id, subject)
    }

    #[test]
    fn new_entry_defaults_to_pending_with_empty_optionals() {
        let e = entry("1", "Write tests");
        assert_eq!(e.status, TaskStatus::Pending);
        assert_eq!(e.description, "");
        assert_eq!(e.owner, None);
        assert!(e.blocked_by.is_empty());
        assert!(e.blocks.is_empty());
        assert!(e.metadata.is_empty());
    }

    #[test]
    fn crud_roundtrip_on_data_structure() {
        let mut list = TaskList::new();
        // create
        list.upsert(entry("1", "a"));
        list.upsert(entry("2", "b"));
        assert_eq!(list.tasks.len(), 2);

        // read
        assert_eq!(list.get("1").map(|t| t.subject.as_str()), Some("a"));
        assert!(list.get("404").is_none());

        // update
        let patch = TaskUpdate {
            status: Some(TaskStatus::InProgress),
            owner: Some(Some("agent-1".to_string())),
            ..Default::default()
        };
        list.update("2", &patch).unwrap();
        let updated = list.get("2").unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
        assert_eq!(updated.owner.as_deref(), Some("agent-1"));

        // remove
        assert_eq!(list.remove("1").map(|t| t.subject), Some("a".to_string()));
        assert_eq!(list.tasks.len(), 1);
        assert!(list.get("1").is_none());
    }

    #[test]
    fn list_sorted_is_numeric_then_stable() {
        let mut list = TaskList::new();
        // Insert out of order; numeric ids must sort numerically (10 > 2).
        list.upsert(entry("10", "ten"));
        list.upsert(entry("2", "two"));
        list.upsert(entry("1", "one"));

        let ids: Vec<String> = list.list_sorted().into_iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids, vec!["1", "2", "10"]);
    }

    #[test]
    fn next_id_is_sequential_and_ignores_non_numeric() {
        let mut list = TaskList::new();
        assert_eq!(list.next_id(), "1");
        list.upsert(entry("1", "a"));
        list.upsert(entry("2", "b"));
        list.upsert(entry("beta", "non-numeric"));
        assert_eq!(list.next_id(), "3");
    }

    #[test]
    fn update_clears_owner_with_some_none() {
        let mut list = TaskList::new();
        let mut e = entry("1", "a");
        e.owner = Some("agent-1".to_string());
        list.upsert(e);

        list.update(
            "1",
            &TaskUpdate {
                owner: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(list.get("1").unwrap().owner, None);
    }

    #[test]
    fn upsert_replaces_existing_id_and_returns_old() {
        let mut list = TaskList::new();
        list.upsert(entry("1", "old"));
        let old = list.upsert(entry("1", "new"));
        assert_eq!(old.map(|t| t.subject), Some("old".to_string()));
        assert_eq!(list.tasks.len(), 1);
        assert_eq!(list.get("1").unwrap().subject, "new");
    }

    #[test]
    fn entry_serializes_and_round_trips() {
        let mut e = entry("7", "Ship it");
        e.description = "details".into();
        e.status = TaskStatus::InProgress;
        e.owner = Some("agent-9".into());
        e.blocked_by = vec!["3".into(), "4".into()];
        e.blocks = vec!["9".into()];
        e.metadata.insert("kind".into(), json!("bug"));

        let list = {
            let mut l = TaskList::new();
            l.upsert(e);
            l
        };

        let json_str = serde_json::to_string(&list).unwrap();
        let back: TaskList = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, list);
        assert_eq!(back.schema_version(), 1);

        // Field-level checks confirm the rich shape survives.
        let e = &back.tasks[0];
        assert_eq!(e.subject, "Ship it");
        assert_eq!(e.owner.as_deref(), Some("agent-9"));
        assert_eq!(e.blocked_by, vec!["3".to_string(), "4".to_string()]);
        assert_eq!(e.metadata.get("kind"), Some(&json!("bug")));
    }

    #[test]
    fn deserialize_partial_entry_uses_defaults() {
        // A minimal JSON object (only required fields) must load with defaults.
        let json_str = r#"{"schema_version":1,"tasks":[{"id":"1","subject":"x","status":"pending"}]}"#;
        let list: TaskList = serde_json::from_str(json_str).unwrap();
        let e = &list.tasks[0];
        assert_eq!(e.description, "");
        assert!(e.owner.is_none());
        assert!(e.metadata.is_empty());
    }

    // ---- TaskStore persistence ----

    fn temp_root() -> PathBuf {
        let unique = format!(
            "task-list-test-{}-{}",
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

    #[test]
    fn store_load_missing_scope_returns_empty_list() {
        let store = TaskStore::new(temp_root());
        let list = store.load("team-a").unwrap();
        assert!(list.tasks.is_empty());
        assert_eq!(list.schema_version(), 1);
    }

    #[test]
    fn store_save_then_load_round_trips() {
        let store = TaskStore::new(temp_root());
        let mut list = TaskList::new();
        let mut e = entry("1", "design");
        e.owner = Some("agent-1".into());
        e.blocked_by = vec!["2".into()];
        list.upsert(e);
        list.upsert(entry("2", "implement"));

        store.save("team-a", &list).unwrap();
        let back = store.load("team-a").unwrap();
        assert_eq!(back, list);
        assert_eq!(back.list_sorted().len(), 2);
    }

    #[test]
    fn store_persists_to_named_scope_file() {
        let root = temp_root();
        let store = TaskStore::new(&root);
        store.save("team-a", &TaskList::new()).unwrap();
        assert!(root.join("team-a.json").exists());
    }

    #[test]
    fn store_isolates_scopes() {
        let store = TaskStore::new(temp_root());
        let mut a = TaskList::new();
        a.upsert(entry("1", "a-task"));
        store.save("team-a", &a).unwrap();

        let mut b = TaskList::new();
        b.upsert(entry("1", "b-task"));
        store.save("team-b", &b).unwrap();

        assert_eq!(store.load("team-a").unwrap().get("1").unwrap().subject, "a-task");
        assert_eq!(store.load("team-b").unwrap().get("1").unwrap().subject, "b-task");
    }

    #[test]
    fn sanitize_scope_rejects_traversal_and_special_chars() {
        assert_eq!(sanitize_scope("team-a"), "team-a");
        assert_eq!(sanitize_scope("team/a/b"), "team_a_b");
        assert_eq!(sanitize_scope(".."), "default");
        assert_eq!(sanitize_scope("with space"), "with_space");
        assert_eq!(sanitize_scope(""), "default");
        // The sanitized name never contains a path separator, so it stays a
        // single component under root.
        assert!(!sanitize_scope("../etc/passwd").contains('/'));
    }

    #[test]
    fn default_root_points_under_config_dir_when_home_set() {
        // Only assert shape when HOME is available; never mutate env in a unit test.
        if std::env::var("HOME").is_ok() {
            let root = TaskStore::default_root().unwrap();
            assert!(root.ends_with("tasks"));
            assert!(root.starts_with(".config/rust-claude-code") || root.to_string_lossy().contains(".config/rust-claude-code"));
        }
    }
}
