//! `EnterWorktree` / `ExitWorktree` — git worktree isolation tools (iteration 52).
//!
//! `EnterWorktree` either creates a fresh linked worktree under
//! `<repo>/.claude/worktrees/<name>` (on a new branch from HEAD) or enters an
//! existing one by path, then repoints the session's working directory into it
//! so subsequent tools (Bash, file edits, …) operate on the isolated checkout.
//!
//! `ExitWorktree` returns the session to the directory it came from. With
//! `action: "keep"` the worktree and branch are left on disk; with
//! `action: "remove"` both are deleted, but only when it is safe — uncommitted
//! changes block removal unless `discard_changes` is set.
//!
//! Both tools mutate [`rust_claude_core::state::AppState`] (`cwd`, `worktree`,
//! `git_context`), so neither is read-only nor concurrency-safe. The git
//! plumbing itself lives in [`rust_claude_core::git`].

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rust_claude_core::git::{self, collect_git_context, ActiveWorktree};
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use serde::Deserialize;

use crate::tool::{Tool, ToolContext, ToolError};

// ── EnterWorktree ──

/// Create or enter a git worktree and switch the session into it.
#[derive(Debug, Clone)]
pub struct EnterWorktreeTool;

#[derive(Debug, Clone, Deserialize)]
struct EnterInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

impl EnterWorktreeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "EnterWorktree".to_string(),
            description: "Create an isolated git worktree at \
                `<repo>/.claude/worktrees/<name>` on a new branch (from HEAD) and switch this \
                session's working directory into it, or enter an existing worktree by `path`. \
                Subsequent tools (Bash, file edits, …) run from the worktree. Call ExitWorktree \
                to leave. Supply exactly one of `name` (create) or `path` (enter existing)."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name for a new worktree. Creates `.claude/worktrees/<name>` and a branch of the same name starting from HEAD."
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to an existing worktree directory to enter."
                    }
                }
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: EnterInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        let app_state = context
            .app_state
            .clone()
            .ok_or_else(|| ToolError::Execution("no app state available".to_string()))?;

        // Snapshot the current cwd and whether we are already in a worktree,
        // then drop the lock for the blocking `git worktree add`.
        let (cwd, already_in_worktree) = {
            let state = app_state.lock().await;
            (state.cwd.clone(), state.worktree.is_some())
        };

        let (path, branch, created) = match (input.name, input.path) {
            (Some(_), Some(_)) => {
                return Err(ToolError::InvalidInput(
                    "provide either 'name' (create) or 'path' (enter existing), not both".into(),
                ));
            }
            (Some(name), None) => {
                if name.trim().is_empty() {
                    return Err(ToolError::InvalidInput("'name' cannot be empty".into()));
                }
                if already_in_worktree {
                    return Err(ToolError::Execution(
                        "already in a worktree; call ExitWorktree before creating a new one".into(),
                    ));
                }
                let repo_root = git::repo_root(&cwd)
                    .ok_or_else(|| ToolError::Execution("not inside a git repository".into()))?;
                let (path, branch) =
                    git::create_worktree(&repo_root, &name).map_err(ToolError::Execution)?;
                (path, branch, true)
            }
            (None, Some(path)) => {
                let (path, branch) = git::enter_existing_worktree(&PathBuf::from(path))
                    .map_err(ToolError::Execution)?;
                (path, branch, false)
            }
            (None, None) => {
                return Err(ToolError::InvalidInput(
                    "provide either 'name' (create a new worktree) or 'path' (enter an existing one)"
                        .into(),
                ));
            }
        };

        // Refresh git context from inside the worktree (blocking) before
        // applying the switch.
        let path_for_ctx = path.clone();
        let git_context = tokio::task::spawn_blocking(move || collect_git_context(&path_for_ctx))
            .await
            .map_err(|e| ToolError::Execution(format!("git context refresh failed: {e}")))?;

        {
            let mut state = app_state.lock().await;
            state.worktree = Some(ActiveWorktree {
                original_cwd: cwd.clone(),
                path: path.clone(),
                branch: branch.clone(),
            });
            state.cwd = path.clone();
            state.git_context = git_context;
        }

        let verb = if created { "Created" } else { "Entered" };
        Ok(ToolResult::success(
            context.tool_use_id,
            format!(
                "{verb} worktree at {} (branch `{branch}`). Subsequent tools run from this \
                 directory; use ExitWorktree to leave.",
                path.display()
            ),
        ))
    }
}

// ── ExitWorktree ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ExitAction {
    Keep,
    Remove,
}

#[derive(Debug, Clone, Deserialize)]
struct ExitInput {
    action: ExitAction,
    #[serde(default)]
    discard_changes: bool,
}

/// Leave the active worktree session, optionally removing it.
#[derive(Debug, Clone)]
pub struct ExitWorktreeTool;

impl ExitWorktreeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "ExitWorktree".to_string(),
            description: "Leave the active worktree session and restore the working directory to \
                where it was before EnterWorktree. `action: \"keep\"` leaves the worktree and its \
                branch on disk. `action: \"remove\"` deletes both, but refuses by default when the \
                worktree has uncommitted/untracked changes or an unmerged branch; set \
                `discard_changes: true` to remove anyway."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["keep", "remove"],
                        "description": "Whether to keep or remove the worktree on exit."
                    },
                    "discard_changes": {
                        "type": "boolean",
                        "default": false,
                        "description": "With `remove`, force-delete modified files and the branch even if uncommitted/unmerged work would be lost."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: ExitInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        let app_state = context
            .app_state
            .clone()
            .ok_or_else(|| ToolError::Execution("no app state available".to_string()))?;

        let active = {
            let state = app_state.lock().await;
            state.worktree.clone()
        }
        .ok_or_else(|| {
            ToolError::Execution("no worktree session is active; nothing to exit".into())
        })?;

        match input.action {
            ExitAction::Keep => {
                restore_session(&app_state, &active.original_cwd).await?;
                Ok(ToolResult::success(
                    context.tool_use_id,
                    format!(
                        "Left worktree at {} (branch `{}` kept). Restored working directory to {}.",
                        active.path.display(),
                        active.branch,
                        active.original_cwd.display()
                    ),
                ))
            }
            ExitAction::Remove => {
                // Uncommitted/untracked changes are the primary safety gate.
                if !input.discard_changes && git::has_uncommitted_changes(&active.path) {
                    return Err(ToolError::Execution(
                        "worktree has uncommitted or untracked changes; set discard_changes=true \
                         to remove anyway"
                            .into(),
                    ));
                }
                let repo_root = git::common_repo_root(&active.path)
                    .or_else(|| git::repo_root(&active.original_cwd))
                    .ok_or_else(|| {
                        ToolError::Execution(
                            "cannot determine repository root for worktree removal".to_string(),
                        )
                    })?;

                // Order is forced: the worktree must be removed before its
                // branch can be deleted (git refuses to delete a checked-out
                // branch).
                git::remove_worktree(&repo_root, &active.path, input.discard_changes)
                    .map_err(ToolError::Execution)?;

                // `-d` (no discard) refuses an unmerged branch — the safety net
                // for committed work that the uncommitted-files check cannot see.
                // The worktree dir is already gone; if the branch survives we
                // still restore the session so it isn't stranded.
                if let Err(branch_err) =
                    git::delete_branch(&repo_root, &active.branch, input.discard_changes)
                {
                    if input.discard_changes {
                        return Err(ToolError::Execution(branch_err));
                    }
                    restore_session(&app_state, &active.original_cwd).await?;
                    return Err(ToolError::Execution(format!(
                        "removed worktree at {} but kept branch `{}` ({branch_err}); \
                         set discard_changes=true to delete it",
                        active.path.display(),
                        active.branch
                    )));
                }

                restore_session(&app_state, &active.original_cwd).await?;
                Ok(ToolResult::success(
                    context.tool_use_id,
                    format!(
                        "Removed worktree at {} and branch `{}`. Restored working directory to {}.",
                        active.path.display(),
                        active.branch,
                        active.original_cwd.display()
                    ),
                ))
            }
        }
    }
}

/// Restore the session to `original_cwd` after leaving a worktree: repoint
/// cwd, clear the worktree state, and refresh git context from the restored
/// directory.
async fn restore_session(
    app_state: &std::sync::Arc<tokio::sync::Mutex<rust_claude_core::state::AppState>>,
    original_cwd: &Path,
) -> Result<(), ToolError> {
    let original_for_ctx = original_cwd.to_path_buf();
    let git_context = tokio::task::spawn_blocking(move || collect_git_context(&original_for_ctx))
        .await
        .map_err(|e| ToolError::Execution(format!("git context refresh failed: {e}")))?;
    let mut state = app_state.lock().await;
    state.cwd = original_cwd.to_path_buf();
    state.worktree = None;
    state.git_context = git_context;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::state::AppState;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command as StdCommand;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn git_available() -> bool {
        StdCommand::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rust-claude-worktree-tool-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Init a temp git repo with one commit so HEAD exists to branch from.
    fn make_git_repo(label: &str) -> PathBuf {
        let dir = unique_temp_dir(label);
        StdCommand::new("git")
            .args(["init"])
            .current_dir(&dir)
            .output()
            .expect("git init");
        for (k, v) in [
            ("user.name", "Test"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            StdCommand::new("git")
                .args(["config", k, v])
                .current_dir(&dir)
                .output()
                .expect("git config");
        }
        fs::write(dir.join("README.md"), "init\n").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&dir)
            .output()
            .expect("git add");
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&dir)
            .output()
            .expect("git commit");
        dir
    }

    fn context_with(app_state: Arc<Mutex<AppState>>) -> ToolContext {
        ToolContext {
            tool_use_id: "t".to_string(),
            app_state: Some(app_state),
            agent_context: None,
            user_question_callback: None,
            ..Default::default()
        }
    }

    fn state_in(repo: &Path) -> Arc<Mutex<AppState>> {
        Arc::new(Mutex::new(AppState::new(repo.to_path_buf())))
    }

    #[tokio::test]
    async fn enter_creates_worktree_and_switches_cwd() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("enter-create");
        let app_state = state_in(&repo);

        let result = EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "topic" }),
                context_with(app_state.clone()),
            )
            .await
            .expect("enter succeeds");

        let state = app_state.lock().await;
        let active = state.worktree.as_ref().expect("worktree set");
        assert_eq!(active.branch, "topic");
        assert!(active.path.ends_with(".claude/worktrees/topic"));
        // cwd switched into the worktree.
        assert_eq!(state.cwd, active.path);
        // original_cwd remembered as the repo root.
        assert_eq!(active.original_cwd, repo);
        drop(state);
        assert!(result.content.contains("Created"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn enter_refuses_when_already_in_worktree() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("enter-twice");
        let app_state = state_in(&repo);
        // First enter succeeds.
        EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "first" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap();
        // Second create must refuse (still inside a worktree session).
        let err = EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "second" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn enter_requires_name_or_path() {
        let repo = make_git_repo("enter-none");
        let app_state = state_in(&repo);
        let err = EnterWorktreeTool::new()
            .execute(serde_json::json!({}), context_with(app_state.clone()))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        let err = EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "a", "path": "b" }),
                context_with(app_state),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn enter_existing_by_path() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("enter-existing");
        // Create a worktree out-of-band, then enter it by path.
        let (wt_path, _) = rust_claude_core::git::create_worktree(&repo, "preexisting").unwrap();
        let app_state = state_in(&repo);

        let result = EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "path": wt_path.to_string_lossy() }),
                context_with(app_state.clone()),
            )
            .await
            .expect("enter existing");

        let state = app_state.lock().await;
        let active = state.worktree.as_ref().unwrap();
        assert_eq!(active.branch, "preexisting");
        assert!(result.content.contains("Entered"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn exit_keep_restores_cwd_and_clears_state() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("exit-keep");
        let app_state = state_in(&repo);
        EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "k" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap();
        let wt_path = app_state.lock().await.worktree.as_ref().unwrap().path.clone();

        let result = ExitWorktreeTool::new()
            .execute(
                serde_json::json!({ "action": "keep" }),
                context_with(app_state.clone()),
            )
            .await
            .expect("exit keep");

        let state = app_state.lock().await;
        assert!(state.worktree.is_none());
        assert_eq!(state.cwd, repo);
        drop(state);
        // Directory + branch preserved.
        assert!(wt_path.exists());
        assert!(result.content.contains("kept"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn exit_remove_clean_deletes_dir_and_branch() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("exit-remove-clean");
        let app_state = state_in(&repo);
        EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "r" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap();
        let wt_path = app_state.lock().await.worktree.as_ref().unwrap().path.clone();
        let branch = app_state.lock().await.worktree.as_ref().unwrap().branch.clone();

        let result = ExitWorktreeTool::new()
            .execute(
                serde_json::json!({ "action": "remove" }),
                context_with(app_state.clone()),
            )
            .await
            .expect("exit remove");

        let state = app_state.lock().await;
        assert!(state.worktree.is_none());
        assert_eq!(state.cwd, repo);
        drop(state);
        assert!(!wt_path.exists(), "worktree dir should be removed");
        // Branch gone.
        let branches = StdCommand::new("git")
            .args(["branch", "--list"])
            .current_dir(&repo)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        assert!(!branches.contains(&branch));
        assert!(result.content.contains("Removed"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn exit_remove_refuses_dirty_worktree() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("exit-remove-dirty");
        let app_state = state_in(&repo);
        EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "d" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap();
        let wt_path = app_state.lock().await.worktree.as_ref().unwrap().path.clone();
        fs::write(wt_path.join("untracked.txt"), "x\n").unwrap();

        let err = ExitWorktreeTool::new()
            .execute(
                serde_json::json!({ "action": "remove" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
        // Worktree dir must survive the refused removal.
        assert!(wt_path.exists());
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn exit_remove_discard_deletes_dirty_worktree() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("exit-remove-discard");
        let app_state = state_in(&repo);
        EnterWorktreeTool::new()
            .execute(
                serde_json::json!({ "name": "dd" }),
                context_with(app_state.clone()),
            )
            .await
            .unwrap();
        let wt_path = app_state.lock().await.worktree.as_ref().unwrap().path.clone();
        fs::write(wt_path.join("untracked.txt"), "x\n").unwrap();

        let result = ExitWorktreeTool::new()
            .execute(
                serde_json::json!({ "action": "remove", "discard_changes": true }),
                context_with(app_state.clone()),
            )
            .await
            .expect("discard remove");
        assert!(!wt_path.exists());
        assert!(result.content.contains("Removed"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn exit_without_active_worktree_errors() {
        let repo = make_git_repo("exit-none");
        let app_state = state_in(&repo);
        let err = ExitWorktreeTool::new()
            .execute(
                serde_json::json!({ "action": "keep" }),
                context_with(app_state),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn tool_metadata_and_flags() {
        let enter = EnterWorktreeTool::new();
        assert_eq!(enter.info().name, "EnterWorktree");
        assert!(!enter.is_read_only());
        assert!(!enter.is_concurrency_safe());

        let exit = ExitWorktreeTool::new();
        assert_eq!(exit.info().name, "ExitWorktree");
        assert!(!exit.is_read_only());
        assert!(!exit.is_concurrency_safe());
    }
}
