//! Team tool family — local-only team orchestration skeleton (iteration 49).
//!
//! Three tools backed by [`rust_claude_core::team::TeamStore`]:
//! - `TeamCreate` — create a local team config directory + metadata.
//! - `TeamDelete` — remove a team's local config (and its mailboxes).
//! - `SendMessage` — append a message to a member's local mailbox file.
//!
//! This is deliberately **local-only**: no teammate processes are spawned and
//! there is no tmux/iTerm/remote backend. `SendMessage` just writes to a
//! mailbox file on disk, so the orchestration flow has a runnable semantic
//! today; a real multi-process backend can be layered in later without
//! changing these tool contracts.
//!
//! As with `task_tools`, the real logic lives in `run(&store, ...)` helpers so
//! it can be tested with a temp-rooted store without touching `$HOME`. Team
//! tools are keyed by team name (not session scope), so they do not need
//! `app_state`.

use async_trait::async_trait;
use rust_claude_core::team::{MailboxMessage, Team, TeamStore};
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use serde::Deserialize;

use crate::tool::{Tool, ToolContext, ToolError};

/// Default sender name when `SendMessage` omits `from`.
const DEFAULT_SENDER: &str = "orchestrator";

/// Resolve the default on-disk team store.
fn default_store() -> Result<TeamStore, ToolError> {
    TeamStore::default_store().map_err(|e| ToolError::Execution(e.to_string()))
}

fn team_not_found(name: &str) -> ToolError {
    ToolError::Execution(format!("team not found: {name}"))
}

// ---------------------------------------------------------------------------
// TeamCreate
// ---------------------------------------------------------------------------

/// Create a new local team. Fails if a team with the same name already exists.
#[derive(Debug, Clone, Default)]
pub struct TeamCreateTool;

#[derive(Debug, Clone, Deserialize)]
struct CreateInput {
    team: String,
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    task_list: Option<String>,
}

impl TeamCreateTool {
    pub fn new() -> Self {
        Self
    }

    fn run(
        store: &TeamStore,
        tool_use_id: &str,
        input: CreateInput,
    ) -> Result<ToolResult, ToolError> {
        if store.exists(&input.team) {
            return Err(ToolError::Execution(format!(
                "team already exists: {}",
                input.team
            )));
        }
        let team = Team {
            name: input.team.clone(),
            members: input.members.clone(),
            agent_type: input.agent_type.clone(),
            task_list: input.task_list.clone(),
        };
        store.save(&team).map_err(io)?;
        Ok(ToolResult::success(
            tool_use_id.to_string(),
            format!("Created team\n{}", format_team(&team)),
        ))
    }
}

#[async_trait]
impl Tool for TeamCreateTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TeamCreate".to_string(),
            description: "Create a local team with named members. A team is a \
                local-only orchestration unit (no teammate processes are spawned): \
                it owns a config directory and per-member mailboxes that SendMessage \
                writes to. Fails if the team already exists."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team": {
                        "type": "string",
                        "description": "Name of the team to create (also the config key)."
                    },
                    "members": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Member names that can receive messages."
                    },
                    "agent_type": {
                        "type": "string",
                        "description": "Optional team-wide default agent type."
                    },
                    "task_list": {
                        "type": "string",
                        "description": "Optional task-list scope (e.g. session id) to bind this team to."
                    }
                },
                "required": ["team"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let store = default_store()?;
        let input: CreateInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&store, &context.tool_use_id, input)
    }
}

// ---------------------------------------------------------------------------
// TeamDelete
// ---------------------------------------------------------------------------

/// Delete a local team, removing its config directory and all mailboxes.
///
/// In this minimal iteration there is no liveness tracking (no teammate
/// processes are spawned), so any persisted team is treated as removable.
#[derive(Debug, Clone, Default)]
pub struct TeamDeleteTool;

#[derive(Debug, Clone, Deserialize)]
struct DeleteInput {
    team: String,
}

impl TeamDeleteTool {
    pub fn new() -> Self {
        Self
    }

    fn run(store: &TeamStore, tool_use_id: &str, input: DeleteInput) -> Result<ToolResult, ToolError> {
        if !store.exists(&input.team) {
            return Err(team_not_found(&input.team));
        }
        store.delete(&input.team).map_err(io)?;
        Ok(ToolResult::success(
            tool_use_id.to_string(),
            format!("Deleted team {}", input.team),
        ))
    }
}

#[async_trait]
impl Tool for TeamDeleteTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TeamDelete".to_string(),
            description: "Delete a local team, removing its config directory and \
                every member mailbox. The team must exist. (No teammate processes \
                are tracked in this minimal version, so deletion always proceeds.)"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team": {
                        "type": "string",
                        "description": "Name of the team to delete."
                    }
                },
                "required": ["team"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let store = default_store()?;
        let input: DeleteInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&store, &context.tool_use_id, input)
    }
}

// ---------------------------------------------------------------------------
// SendMessage
// ---------------------------------------------------------------------------

/// Append a message to a team member's local mailbox file.
#[derive(Debug, Clone, Default)]
pub struct SendMessageTool;

#[derive(Debug, Clone, Deserialize)]
struct SendInput {
    team: String,
    member: String,
    message: String,
    #[serde(default)]
    from: Option<String>,
}

impl SendMessageTool {
    pub fn new() -> Self {
        Self
    }

    fn run(store: &TeamStore, tool_use_id: &str, input: SendInput) -> Result<ToolResult, ToolError> {
        let team = store.load(&input.team).map_err(io)?.ok_or_else(|| team_not_found(&input.team))?;
        if !team.has_member(&input.member) {
            return Err(ToolError::Execution(format!(
                "team {} has no member: {}",
                input.team, input.member
            )));
        }
        let sender = input.from.as_deref().unwrap_or(DEFAULT_SENDER);
        let message = store
            .append_message(&input.team, &input.member, sender, &input.message)
            .map_err(io)?;
        Ok(ToolResult::success(
            tool_use_id.to_string(),
            format!(
                "Sent message to {}/{}\n{}",
                input.team,
                input.member,
                format_message(&message)
            ),
        ))
    }
}

#[async_trait]
impl Tool for SendMessageTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "SendMessage".to_string(),
            description: "Append a message to a team member's local mailbox. The \
                team and member must already exist (create them with TeamCreate). \
                In this minimal local version the mailbox is a file on disk; no \
                teammate process is notified."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team": {
                        "type": "string",
                        "description": "Name of the team the member belongs to."
                    },
                    "member": {
                        "type": "string",
                        "description": "Name of the recipient member."
                    },
                    "message": {
                        "type": "string",
                        "description": "Message body to deliver."
                    },
                    "from": {
                        "type": "string",
                        "description": "Optional sender name. Defaults to \"orchestrator\"."
                    }
                },
                "required": ["team", "member", "message"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let store = default_store()?;
        let input: SendInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&store, &context.tool_use_id, input)
    }
}

// ---------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------

fn format_team(team: &Team) -> String {
    let mut lines = vec![format!("  name: {}", team.name)];
    if team.members.is_empty() {
        lines.push("  members: (none)".to_string());
    } else {
        lines.push(format!("  members: {}", team.members.join(", ")));
    }
    if let Some(agent_type) = &team.agent_type {
        lines.push(format!("  agent_type: {}", agent_type));
    }
    if let Some(task_list) = &team.task_list {
        lines.push(format!("  task_list: {}", task_list));
    }
    lines.join("\n")
}

fn format_message(message: &MailboxMessage) -> String {
    format!(
        "  seq: {}\n  from: {}\n  content: {}",
        message.seq, message.from, message.content
    )
}

fn io(error: rust_claude_core::team::TeamStoreError) -> ToolError {
    ToolError::Execution(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let unique = format!(
            "team-tools-test-{}-{}",
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

    fn store() -> TeamStore {
        TeamStore::new(temp_root())
    }

    // ---- TeamCreate ----

    #[test]
    fn team_create_persists_and_lists_members() {
        let store = store();
        let r = TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec!["w1".into(), "w2".into()],
                agent_type: Some("general-purpose".into()),
                task_list: None,
            },
        )
        .unwrap();
        assert!(r.content.contains("Created team"));
        assert!(r.content.contains("members: w1, w2"));
        assert!(r.content.contains("agent_type: general-purpose"));

        let team = store.load("alpha").unwrap().unwrap();
        assert_eq!(team.members, vec!["w1".to_string(), "w2".to_string()]);
    }

    #[test]
    fn team_create_rejects_duplicate() {
        let store = store();
        TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec![],
                agent_type: None,
                task_list: None,
            },
        )
        .unwrap();
        let err = TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec![],
                agent_type: None,
                task_list: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    // ---- TeamDelete ----

    #[test]
    fn team_delete_removes_team() {
        let store = store();
        TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec!["w1".into()],
                agent_type: None,
                task_list: None,
            },
        )
        .unwrap();
        let r = TeamDeleteTool::run(
            &store,
            "t",
            DeleteInput {
                team: "alpha".into(),
            },
        )
        .unwrap();
        assert!(r.content.contains("Deleted team alpha"));
        assert!(!store.exists("alpha"));
    }

    #[test]
    fn team_delete_errors_on_missing() {
        let store = store();
        let err = TeamDeleteTool::run(
            &store,
            "t",
            DeleteInput {
                team: "ghost".into(),
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    // ---- SendMessage ----

    #[test]
    fn send_message_writes_to_member_mailbox() {
        let store = store();
        TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec!["w1".into()],
                agent_type: None,
                task_list: None,
            },
        )
        .unwrap();
        let r = SendMessageTool::run(
            &store,
            "t",
            SendInput {
                team: "alpha".into(),
                member: "w1".into(),
                message: "please review PR 42".into(),
                from: Some("lead".into()),
            },
        )
        .unwrap();
        assert!(r.content.contains("Sent message to alpha/w1"));
        assert!(r.content.contains("from: lead"));
        assert!(r.content.contains("please review PR 42"));

        // Persisted to the member's mailbox.
        let mailbox = store.read_mailbox("alpha", "w1").unwrap();
        assert_eq!(mailbox.len(), 1);
        assert_eq!(mailbox[0].content, "please review PR 42");
        assert_eq!(mailbox[0].from, "lead");
        assert_eq!(mailbox[0].seq, 1);
    }

    #[test]
    fn send_message_defaults_sender_to_orchestrator() {
        let store = store();
        TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec!["w1".into()],
                agent_type: None,
                task_list: None,
            },
        )
        .unwrap();
        SendMessageTool::run(
            &store,
            "t",
            SendInput {
                team: "alpha".into(),
                member: "w1".into(),
                message: "hi".into(),
                from: None,
            },
        )
        .unwrap();
        assert_eq!(store.read_mailbox("alpha", "w1").unwrap()[0].from, "orchestrator");
    }

    #[test]
    fn send_message_errors_for_unknown_team() {
        let store = store();
        let err = SendMessageTool::run(
            &store,
            "t",
            SendInput {
                team: "ghost".into(),
                member: "w1".into(),
                message: "hi".into(),
                from: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    #[test]
    fn send_message_errors_for_unknown_member() {
        let store = store();
        TeamCreateTool::run(
            &store,
            "t",
            CreateInput {
                team: "alpha".into(),
                members: vec!["w1".into()],
                agent_type: None,
                task_list: None,
            },
        )
        .unwrap();
        let err = SendMessageTool::run(
            &store,
            "t",
            SendInput {
                team: "alpha".into(),
                member: "stranger".into(),
                message: "hi".into(),
                from: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
    }

    // ---- schema sanity ----

    #[test]
    fn tools_expose_expected_names() {
        assert_eq!(TeamCreateTool::new().info().name, "TeamCreate");
        assert!(!TeamCreateTool::new().is_read_only());
        assert_eq!(TeamDeleteTool::new().info().name, "TeamDelete");
        assert!(!TeamDeleteTool::new().is_read_only());
        assert_eq!(SendMessageTool::new().info().name, "SendMessage");
        assert!(!SendMessageTool::new().is_read_only());
    }
}
