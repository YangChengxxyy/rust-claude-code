//! Task tool family — independent tools backed by the persisted task-list store.
//!
//! These four tools (`TaskCreate` / `TaskGet` / `TaskList` / `TaskUpdate`) are
//! the original-style task management surface, built on top of
//! [`rust_claude_core::task_list`]. They are distinct from the legacy in-memory
//! [`crate::TaskTool`]: each call loads the task list for the current scope
//! (the session id until Team support lands in iteration 49) from disk, mutates
//! it, and saves it back, so tasks persist across runs.
//!
//! The real work lives in `run(&store, &scope, ...)` helpers so it can be tested
//! with a temp-rooted store without touching `$HOME`.

use async_trait::async_trait;
use rust_claude_core::state::TaskStatus;
use rust_claude_core::task_list::{TaskListEntry, TaskStore, TaskUpdate};
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use serde::Deserialize;

use crate::tool::{Tool, ToolContext, ToolError};

/// Resolve the persisted task store + the current scope (session id) for a tool
/// invocation. Requires `app_state` in the context.
async fn resolve_store_and_scope(
    context: &ToolContext,
) -> Result<(TaskStore, String), ToolError> {
    let app_state = context
        .app_state
        .clone()
        .ok_or_else(|| ToolError::Execution("Task tools require app_state".to_string()))?;
    let scope = app_state.lock().await.session.id.clone();
    let store = TaskStore::default_store().map_err(|e| ToolError::Execution(e.to_string()))?;
    Ok((store, scope))
}

fn status_str(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Completed => "completed",
        TaskStatus::Cancelled => "cancelled",
    }
}

/// Render a task entry for tool output.
fn format_entry(entry: &TaskListEntry) -> String {
    let mut lines = vec![
        format!("Task {}", entry.id),
        format!("  subject: {}", entry.subject),
        format!("  status: {}", status_str(entry.status)),
    ];
    if !entry.description.is_empty() {
        lines.push(format!("  description: {}", entry.description));
    }
    if let Some(owner) = &entry.owner {
        lines.push(format!("  owner: {}", owner));
    }
    if !entry.blocked_by.is_empty() {
        lines.push(format!("  blocked_by: {}", entry.blocked_by.join(", ")));
    }
    if !entry.blocks.is_empty() {
        lines.push(format!("  blocks: {}", entry.blocks.join(", ")));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// TaskCreate
// ---------------------------------------------------------------------------

/// Create a new task in the current scope's task list.
#[derive(Debug, Clone, Default)]
pub struct TaskCreateTool;

#[derive(Debug, Clone, Deserialize)]
struct CreateInput {
    subject: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    blocked_by: Option<Vec<String>>,
}

impl TaskCreateTool {
    pub fn new() -> Self {
        Self
    }

    fn run(
        store: &TaskStore,
        scope: &str,
        tool_use_id: &str,
        input: CreateInput,
    ) -> Result<ToolResult, ToolError> {
        let mut list = store.load(scope).map_err(io)?;
        let id = list.next_id();
        let mut entry = TaskListEntry::new(id.clone(), input.subject);
        if let Some(desc) = input.description {
            entry.description = desc;
        }
        if let Some(blocked_by) = input.blocked_by {
            entry.blocked_by = blocked_by;
        }
        list.upsert(entry.clone());
        store.save(scope, &list).map_err(io)?;
        Ok(ToolResult::success(
            tool_use_id.to_string(),
            format!("Created task\n{}", format_entry(&entry)),
        ))
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TaskCreate".to_string(),
            description: "Create a new task in the current session's task list. \
                Tasks persist across the session and are identified by a sequential \
                numeric id. Use TaskUpdate to change status, owner, or dependencies."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Short imperative title of the task."
                    },
                    "description": {
                        "type": "string",
                        "description": "Longer details, requirements, or context."
                    },
                    "blocked_by": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "IDs of tasks that must complete before this one."
                    }
                },
                "required": ["subject"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let (store, scope) = resolve_store_and_scope(&context).await?;
        let input: CreateInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&store, &scope, &context.tool_use_id, input)
    }
}

// ---------------------------------------------------------------------------
// TaskGet
// ---------------------------------------------------------------------------

/// Fetch a single task by id.
#[derive(Debug, Clone, Default)]
pub struct TaskGetTool;

#[derive(Debug, Clone, Deserialize)]
struct GetInput {
    id: String,
}

impl TaskGetTool {
    pub fn new() -> Self {
        Self
    }

    fn run(
        store: &TaskStore,
        scope: &str,
        tool_use_id: &str,
        id: &str,
    ) -> Result<ToolResult, ToolError> {
        let list = store.load(scope).map_err(io)?;
        match list.get(id) {
            Some(entry) => Ok(ToolResult::success(
                tool_use_id.to_string(),
                format_entry(entry),
            )),
            None => Err(ToolError::Execution(format!("task not found: {id}"))),
        }
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TaskGet".to_string(),
            description: "Fetch a single task by its id from the current session's \
                task list, including its status, owner, and dependencies."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "The task id." }
                },
                "required": ["id"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let (store, scope) = resolve_store_and_scope(&context).await?;
        let input: GetInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&store, &scope, &context.tool_use_id, &input.id)
    }
}

// ---------------------------------------------------------------------------
// TaskList
// ---------------------------------------------------------------------------

/// List all tasks in the current scope, sorted by id.
#[derive(Debug, Clone, Default)]
pub struct TaskListTool;

impl TaskListTool {
    pub fn new() -> Self {
        Self
    }

    fn run(store: &TaskStore, scope: &str, tool_use_id: &str) -> Result<ToolResult, ToolError> {
        let list = store.load(scope).map_err(io)?;
        let entries = list.list_sorted();
        if entries.is_empty() {
            return Ok(ToolResult::success(tool_use_id.to_string(), "No tasks"));
        }
        let body = entries
            .iter()
            .map(|e| format_entry(e))
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(ToolResult::success(tool_use_id.to_string(), body))
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TaskList".to_string(),
            description: "List all tasks in the current session's task list, sorted \
                by id (numeric order). Returns each task's status, owner, and \
                dependency graph."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let (store, scope) = resolve_store_and_scope(&context).await?;
        Self::run(&store, &scope, &context.tool_use_id)
    }
}

// ---------------------------------------------------------------------------
// TaskUpdate
// ---------------------------------------------------------------------------

/// Update an existing task's fields.
#[derive(Debug, Clone, Default)]
pub struct TaskUpdateTool;

#[derive(Debug, Clone, Default, Deserialize)]
struct UpdateInput {
    id: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: Option<TaskStatus>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    blocked_by: Option<Vec<String>>,
    #[serde(default)]
    blocks: Option<Vec<String>>,
}

impl UpdateInput {
    fn into_patch(self) -> TaskUpdate {
        TaskUpdate {
            subject: self.subject,
            description: self.description,
            status: self.status,
            // `Some(value)` sets the owner; `None` (absent) leaves it unchanged.
            owner: self.owner.map(Some),
            blocked_by: self.blocked_by,
            blocks: self.blocks,
            metadata: None,
        }
    }
}

impl TaskUpdateTool {
    pub fn new() -> Self {
        Self
    }

    fn run(
        store: &TaskStore,
        scope: &str,
        tool_use_id: &str,
        input: UpdateInput,
    ) -> Result<ToolResult, ToolError> {
        let id = input.id.clone();
        let patch = input.into_patch();
        let mut list = store.load(scope).map_err(io)?;
        if list.get(&id).is_none() {
            return Err(ToolError::Execution(format!("task not found: {id}")));
        }
        list.update(&id, &patch);
        let updated = list.get(&id).cloned().expect("entry present after update");
        store.save(scope, &list).map_err(io)?;
        Ok(ToolResult::success(
            tool_use_id.to_string(),
            format!("Updated task\n{}", format_entry(&updated)),
        ))
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TaskUpdate".to_string(),
            description: "Update an existing task in the current session's task list. \
                Only provided fields are changed. Use blocked_by/blocks to express \
                dependencies between tasks."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "The task id to update." },
                    "subject": { "type": "string" },
                    "description": { "type": "string" },
                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"] },
                    "owner": { "type": "string", "description": "Agent that owns the task." },
                    "blocked_by": { "type": "array", "items": { "type": "string" }, "description": "IDs of tasks that must complete first." },
                    "blocks": { "type": "array", "items": { "type": "string" }, "description": "IDs of tasks waiting on this one." }
                },
                "required": ["id"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let (store, scope) = resolve_store_and_scope(&context).await?;
        let input: UpdateInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&store, &scope, &context.tool_use_id, input)
    }
}

fn io(error: rust_claude_core::task_list::TaskStoreError) -> ToolError {
    ToolError::Execution(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let unique = format!(
            "task-tools-test-{}-{}",
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

    fn store() -> TaskStore {
        TaskStore::new(temp_root())
    }

    const SCOPE: &str = "test-scope";

    // ---- TaskCreate ----

    #[test]
    fn task_create_assigns_sequential_id_and_persists() {
        let store = store();
        let r1 = TaskCreateTool::run(
            &store,
            SCOPE,
            "t1",
            CreateInput {
                subject: "first".into(),
                description: Some("details".into()),
                blocked_by: None,
            },
        )
        .unwrap();
        assert!(r1.content.contains("Task 1"));
        assert!(r1.content.contains("subject: first"));

        let r2 = TaskCreateTool::run(
            &store,
            SCOPE,
            "t2",
            CreateInput {
                subject: "second".into(),
                description: None,
                blocked_by: None,
            },
        )
        .unwrap();
        assert!(r2.content.contains("Task 2"));

        // Persisted: a fresh load sees both tasks.
        let list = store.load(SCOPE).unwrap();
        assert_eq!(list.tasks.len(), 2);
        assert_eq!(list.get("1").unwrap().description, "details");
    }

    #[test]
    fn task_create_records_blocked_by() {
        let store = store();
        TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "parent".into(),
                description: None,
                blocked_by: None,
            },
        )
        .unwrap();
        let r = TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "child".into(),
                description: None,
                blocked_by: Some(vec!["1".into()]),
            },
        )
        .unwrap();
        assert!(r.content.contains("blocked_by: 1"));
    }

    // ---- TaskGet ----

    #[test]
    fn task_get_returns_created_task() {
        let store = store();
        TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "find me".into(),
                description: None,
                blocked_by: None,
            },
        )
        .unwrap();
        let r = TaskGetTool::run(&store, SCOPE, "t", "1").unwrap();
        assert!(r.content.contains("find me"));
    }

    #[test]
    fn task_get_errors_on_missing_id() {
        let store = store();
        let err = TaskGetTool::run(&store, SCOPE, "t", "404").unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    // ---- TaskList ----

    #[test]
    fn task_list_handles_empty_scope() {
        let store = store();
        let r = TaskListTool::run(&store, SCOPE, "t").unwrap();
        assert_eq!(r.content, "No tasks");
    }

    #[test]
    fn task_list_is_sorted_by_numeric_id() {
        let store = store();
        // Insert in an order that would mis-sort lexicographically (10 < 2).
        for subject in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"] {
            TaskCreateTool::run(
                &store,
                SCOPE,
                "t",
                CreateInput {
                    subject: subject.into(),
                    description: None,
                    blocked_by: None,
                },
            )
            .unwrap();
        }
        let r = TaskListTool::run(&store, SCOPE, "t").unwrap();
        // First listed task must be "1", and "2" must come before "10".
        assert!(r.content.starts_with("Task 1\n"));
        let pos2 = r.content.find("Task 2\n").unwrap();
        let pos10 = r.content.find("Task 10\n").unwrap();
        assert!(pos2 < pos10);
    }

    #[test]
    fn task_list_shows_blocked_by_semantics() {
        let store = store();
        TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "do thing".into(),
                description: None,
                blocked_by: None,
            },
        )
        .unwrap();
        TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "after thing".into(),
                description: None,
                blocked_by: Some(vec!["1".into()]),
            },
        )
        .unwrap();
        let r = TaskListTool::run(&store, SCOPE, "t").unwrap();
        assert!(r.content.contains("blocked_by: 1"));
    }

    // ---- TaskUpdate ----

    #[test]
    fn task_update_changes_status_subject_and_dependencies() {
        let store = store();
        TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "orig".into(),
                description: None,
                blocked_by: None,
            },
        )
        .unwrap();
        let r = TaskUpdateTool::run(
            &store,
            SCOPE,
            "t",
            UpdateInput {
                id: "1".into(),
                subject: Some("renamed".into()),
                description: None,
                status: Some(TaskStatus::InProgress),
                owner: Some("agent-7".into()),
                blocked_by: Some(vec![]),
                blocks: Some(vec!["2".into()]),
            },
        )
        .unwrap();
        assert!(r.content.contains("subject: renamed"));
        assert!(r.content.contains("status: in_progress"));
        assert!(r.content.contains("owner: agent-7"));
        assert!(r.content.contains("blocks: 2"));

        // Persisted.
        let entry = store.load(SCOPE).unwrap().get("1").cloned().unwrap();
        assert_eq!(entry.status, TaskStatus::InProgress);
        assert_eq!(entry.subject, "renamed");
        assert_eq!(entry.blocks, vec!["2".to_string()]);
    }

    #[test]
    fn task_update_partial_leaves_other_fields_unchanged() {
        let store = store();
        TaskCreateTool::run(
            &store,
            SCOPE,
            "t",
            CreateInput {
                subject: "keep".into(),
                description: Some("desc".into()),
                blocked_by: None,
            },
        )
        .unwrap();
        TaskUpdateTool::run(
            &store,
            SCOPE,
            "t",
            UpdateInput {
                id: "1".into(),
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
        let entry = store.load(SCOPE).unwrap().get("1").cloned().unwrap();
        assert_eq!(entry.status, TaskStatus::Completed);
        // Untouched fields preserved.
        assert_eq!(entry.subject, "keep");
        assert_eq!(entry.description, "desc");
    }

    #[test]
    fn task_update_errors_on_missing_id() {
        let store = store();
        let err = TaskUpdateTool::run(
            &store,
            SCOPE,
            "t",
            UpdateInput {
                id: "404".into(),
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    // ---- execute() path (resolves store from app_state) ----

    #[tokio::test]
    async fn execute_errors_when_app_state_missing() {
        // No env mutation: just verifies the app_state guard in execute().
        let err = TaskListTool::new()
            .execute(
                serde_json::json!({}),
                ToolContext {
                    tool_use_id: "t".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    // ---- schema sanity ----

    #[test]
    fn tools_expose_expected_names_and_read_only_flags() {
        assert_eq!(TaskCreateTool::new().info().name, "TaskCreate");
        assert!(!TaskCreateTool::new().is_read_only());

        assert_eq!(TaskGetTool::new().info().name, "TaskGet");
        assert!(TaskGetTool::new().is_read_only());
        assert!(TaskGetTool::new().is_concurrency_safe());

        assert_eq!(TaskListTool::new().info().name, "TaskList");
        assert!(TaskListTool::new().is_read_only());
        assert!(TaskListTool::new().is_concurrency_safe());

        assert_eq!(TaskUpdateTool::new().info().name, "TaskUpdate");
        assert!(!TaskUpdateTool::new().is_read_only());
    }
}
