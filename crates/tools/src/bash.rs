use std::path::{Path, PathBuf};
use std::time::Instant;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::sandbox::{platform_adapter, SandboxCommandSpec};
use crate::tool::{Tool, ToolContext, ToolError};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_OUTPUT_LEN: usize = 8_000;
const CWD_MARKER_PREFIX: &str = "__RUST_CLAUDE_FINAL_CWD_";

#[derive(Debug, Clone, Default)]
pub struct BashTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct BashToolInput {
    command: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    workdir: Option<PathBuf>,
}

impl BashTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_input(input: serde_json::Value) -> Result<BashToolInput, ToolError> {
        let input: BashToolInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        if input.command.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "command cannot be empty".to_string(),
            ));
        }

        Ok(input)
    }

    fn is_dangerous_command(command: &str) -> bool {
        let normalized = command.to_ascii_lowercase();
        normalized.contains("rm -rf /")
            || normalized.contains("sudo ")
            || normalized.starts_with("sudo")
    }

    fn truncate_output(output: &str) -> (String, bool) {
        if output.len() <= MAX_OUTPUT_LEN {
            return (output.to_string(), false);
        }

        let head_budget = MAX_OUTPUT_LEN / 2;
        let tail_budget = MAX_OUTPUT_LEN - head_budget;

        // Find a char-boundary-safe split point for head (walk backwards)
        let head_end = {
            let mut end = head_budget.min(output.len());
            while end > 0 && !output.is_char_boundary(end) {
                end -= 1;
            }
            end
        };
        // Find a char-boundary-safe split point for tail (walk forwards)
        let tail_start = {
            let mut start = output.len().saturating_sub(tail_budget);
            while start < output.len() && !output.is_char_boundary(start) {
                start += 1;
            }
            start
        };
        let head = &output[..head_end];
        let tail = &output[tail_start..];

        (format!("{head}\n... output truncated ...\n{tail}"), true)
    }

    fn command_with_cwd_capture(command: &str, marker: &str) -> String {
        format!(
            "{{\n{command}\n}}\n__rust_claude_status=$?\n__rust_claude_pwd=$(pwd -P 2>/dev/null || pwd)\nprintf '%s%s\\n' '{marker}' \"$__rust_claude_pwd\" >&2\nexit $__rust_claude_status"
        )
    }

    fn split_cwd_marker(stderr: &[u8], marker: &str) -> (String, Option<PathBuf>) {
        let stderr = String::from_utf8_lossy(stderr).to_string();
        let Some(marker_start) = stderr.rfind(marker) else {
            return (stderr, None);
        };

        let visible_stderr = stderr[..marker_start].to_string();
        let marker_tail = &stderr[marker_start + marker.len()..];
        let cwd = marker_tail
            .lines()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);

        (visible_stderr, cwd)
    }

    async fn resolve_start_dir(
        explicit_workdir: Option<&Path>,
        context: &ToolContext,
    ) -> Result<Option<PathBuf>, ToolError> {
        if let Some(workdir) = explicit_workdir {
            return Ok(Some(workdir.to_path_buf()));
        }

        if let Some(app_state) = &context.app_state {
            let state = app_state.lock().await;
            return Ok(Some(state.cwd.clone()));
        }

        Ok(None)
    }

    async fn update_session_cwd(context: &ToolContext, final_cwd: Option<PathBuf>) {
        let (Some(app_state), Some(final_cwd)) = (&context.app_state, final_cwd) else {
            return;
        };

        let Ok(cwd) = final_cwd.canonicalize() else {
            return;
        };

        if !cwd.is_dir() {
            return;
        }

        let mut state = app_state.lock().await;
        state.cwd = cwd;
    }

    async fn sandbox_config(
        context: &ToolContext,
    ) -> Option<(rust_claude_core::sandbox::SandboxConfig, PathBuf)> {
        let app_state = context.app_state.as_ref()?;
        let state = app_state.lock().await;
        if state.config.sandbox.enabled {
            Some((state.config.sandbox.clone(), state.cwd.clone()))
        } else {
            None
        }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Bash".to_string(),
            description: "Execute a shell command".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeout_ms": { "type": "integer", "minimum": 1 },
                    "workdir": { "type": "string" }
                },
                "required": ["command"]
            }),
        }
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input = Self::validate_input(input)?;
        if Self::is_dangerous_command(&input.command) {
            return Err(ToolError::Execution(
                "dangerous command detected".to_string(),
            ));
        }

        let started_at = Instant::now();
        let marker = format!("{CWD_MARKER_PREFIX}{}__", uuid::Uuid::new_v4());
        let shell_command = Self::command_with_cwd_capture(&input.command, &marker);
        let start_dir = Self::resolve_start_dir(input.workdir.as_deref(), &context).await?;
        let sandbox_config = Self::sandbox_config(&context).await;
        let mut command = if let Some((sandbox_config, fallback_allowed_path)) = sandbox_config {
            let adapter = platform_adapter();
            if let rust_claude_core::sandbox::SandboxAdapterAvailability::Unavailable { reason } =
                adapter.availability()
            {
                return Err(ToolError::Execution(format!(
                    "sandbox unsupported: {reason}"
                )));
            }
            let spec = SandboxCommandSpec::new(
                "sh",
                vec!["-c".to_string(), shell_command],
                start_dir,
                &sandbox_config,
                &fallback_allowed_path,
            );
            adapter
                .wrap(&spec)
                .map_err(|error| ToolError::Execution(format!("sandbox unsupported: {error}")))?
        } else {
            let mut command = Command::new("sh");
            command.kill_on_drop(true);
            command.arg("-c").arg(shell_command);
            if let Some(start_dir) = start_dir {
                command.current_dir(start_dir);
            }
            command
        };

        let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let output = timeout(Duration::from_millis(timeout_ms), command.output())
            .await
            .map_err(|_| ToolError::Execution("command timed out".to_string()))?
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        let duration_ms = started_at.elapsed().as_millis() as u64;
        let (visible_stderr, final_cwd) = Self::split_cwd_marker(&output.stderr, &marker);
        Self::update_session_cwd(&context, final_cwd).await;
        let combined_output = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            visible_stderr
        );
        let (content, truncated) = Self::truncate_output(&combined_output);

        let result = if output.status.success() {
            ToolResult::success(context.tool_use_id, content)
        } else {
            ToolResult::error(
                context.tool_use_id,
                if content.trim().is_empty() {
                    format!("command exited with status {}", output.status)
                } else {
                    content
                },
            )
        }
        .with_duration(duration_ms);

        Ok(if truncated {
            result.with_truncated()
        } else {
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;
    use rust_claude_core::state::AppState;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{sleep, Duration};

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rust-claude-tools-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }

    fn context_with_state(app_state: Arc<Mutex<AppState>>) -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            app_state: Some(app_state),
            agent_context: None,
            user_question_callback: None,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_bash_executes_simple_command() {
        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "command": "printf hello" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "hello");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool::new();
        let error = tool
            .execute(
                serde_json::json!({ "command": "sleep 1", "timeout_ms": 10 }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::Execution(message) if message.contains("timed out")));
    }

    #[tokio::test]
    async fn test_bash_abort_kills_child_process() {
        let marker = format!("rust-claude-bash-abort-{}", uuid::Uuid::new_v4());
        let path = std::env::temp_dir().join(&marker);
        let command = format!("sleep 5; printf done > {}", path.display());
        let tool = BashTool::new();
        let handle = tokio::spawn(async move {
            let _ = tool
                .execute(
                    serde_json::json!({ "command": command, "timeout_ms": 10_000 }),
                    ToolContext {
                        tool_use_id: "tool_abort".to_string(),
                        app_state: None,
                        agent_context: None,
                        user_question_callback: None,
                        ..Default::default()
                    },
                )
                .await;
        });

        sleep(Duration::from_millis(100)).await;
        handle.abort();
        sleep(Duration::from_millis(200)).await;

        assert!(!path.exists(), "aborted Bash child should not keep running");
        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_bash_timeout_does_not_update_session_cwd() {
        let initial_dir = unique_temp_dir("timeout-initial");
        let target_dir = unique_temp_dir("timeout-target");
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(initial_dir.clone())));

        let tool = BashTool::new();
        let error = tool
            .execute(
                serde_json::json!({
                    "command": format!("cd {} && sleep 1", target_dir.display()),
                    "timeout_ms": 10
                }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::Execution(message) if message.contains("timed out")));
        assert_eq!(app_state.lock().await.cwd, initial_dir);
    }

    #[tokio::test]
    async fn test_bash_respects_workdir_and_updates_session_cwd() {
        let temp_dir = unique_temp_dir("workdir");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();
        let file_path = temp_dir.join("sample.txt");
        tokio::fs::write(&file_path, "hello").await.unwrap();
        let initial_dir = unique_temp_dir("workdir-initial");
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(initial_dir)));

        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "command": "ls", "workdir": temp_dir }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap();

        assert!(result.content.contains("sample.txt"));
        assert_eq!(app_state.lock().await.cwd, temp_dir.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_bash_starts_from_session_cwd_by_default() {
        let session_dir = unique_temp_dir("session-default");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();
        tokio::fs::write(session_dir.join("session.txt"), "hello")
            .await
            .unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(session_dir.clone())));

        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "command": "pwd && ls" }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap();

        assert!(result.content.contains(&session_dir.display().to_string()));
        assert!(result.content.contains("session.txt"));
        assert_eq!(
            app_state.lock().await.cwd,
            session_dir.canonicalize().unwrap()
        );
    }

    #[tokio::test]
    async fn test_bash_cd_affects_later_commands() {
        let initial_dir = unique_temp_dir("cd-initial");
        let target_dir = unique_temp_dir("cd-target");
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();
        tokio::fs::write(target_dir.join("target.txt"), "hello")
            .await
            .unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(initial_dir)));

        let tool = BashTool::new();
        let cd_result = tool
            .execute(
                serde_json::json!({ "command": format!("cd {} && pwd", target_dir.display()) }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap();
        assert!(cd_result
            .content
            .contains(&target_dir.display().to_string()));
        assert_eq!(
            app_state.lock().await.cwd,
            target_dir.canonicalize().unwrap()
        );

        let later_result = tool
            .execute(
                serde_json::json!({ "command": "ls" }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap();
        assert!(later_result.content.contains("target.txt"));
    }

    #[tokio::test]
    async fn test_bash_nonzero_exit_still_updates_session_cwd() {
        let initial_dir = unique_temp_dir("nonzero-initial");
        let target_dir = unique_temp_dir("nonzero-target");
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(initial_dir)));

        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": format!("cd {} && printf changed && false", target_dir.display())
                }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert_eq!(result.content, "changed");
        assert_eq!(
            app_state.lock().await.cwd,
            target_dir.canonicalize().unwrap()
        );
    }

    #[tokio::test]
    async fn test_invalid_final_directory_does_not_update_session_cwd() {
        let initial_dir = unique_temp_dir("invalid-initial");
        let missing_dir = unique_temp_dir("invalid-missing");
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(initial_dir.clone())));
        let context = context_with_state(app_state.clone());

        BashTool::update_session_cwd(&context, Some(missing_dir)).await;

        assert_eq!(app_state.lock().await.cwd, initial_dir);
    }

    #[test]
    fn test_cwd_marker_is_removed_from_visible_stderr() {
        let marker = "__RUST_CLAUDE_FINAL_CWD_TEST__";
        let stderr = format!("visible error{marker}/tmp\n");
        let (visible, cwd) = BashTool::split_cwd_marker(stderr.as_bytes(), marker);

        assert_eq!(visible, "visible error");
        assert_eq!(cwd, Some(PathBuf::from("/tmp")));
    }

    #[tokio::test]
    async fn test_bash_truncates_long_output() {
        let tool = BashTool::new();
        let long_output = "x".repeat(MAX_OUTPUT_LEN + 100);
        let command = format!("python3 - <<'PY'\nprint('{}', end='')\nPY", long_output);

        let result = tool
            .execute(
                serde_json::json!({ "command": command }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(result.metadata.truncated);
        assert!(result.content.contains("... output truncated ..."));
        assert!(result.content.len() < long_output.len());
    }

    #[test]
    fn test_bash_detects_dangerous_command() {
        assert!(BashTool::is_dangerous_command("rm -rf /"));
        assert!(BashTool::is_dangerous_command("sudo rm file"));
        assert!(!BashTool::is_dangerous_command("ls -la"));
    }

    #[tokio::test]
    async fn test_bash_sandbox_disabled_preserves_execution() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::env::temp_dir())));
        let result = BashTool::new()
            .execute(
                serde_json::json!({ "command": "printf unsandboxed" }),
                context_with_state(app_state),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "unsandboxed");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_bash_sandbox_enabled_fails_closed_when_runtime_missing() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::env::temp_dir())));
        {
            let mut state = app_state.lock().await;
            state.config.sandbox.enabled = true;
        }

        let availability = crate::sandbox::platform_adapter().availability();
        if availability.is_available() {
            return;
        }

        let error = BashTool::new()
            .execute(
                serde_json::json!({ "command": "printf should-not-run" }),
                context_with_state(app_state),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(error, ToolError::Execution(message) if message.contains("sandbox unsupported"))
        );
    }

    #[tokio::test]
    async fn test_bash_sandbox_final_cwd_when_runtime_available() {
        let availability = crate::sandbox::platform_adapter().availability();
        if !availability.is_available() {
            return;
        }

        let initial_dir = unique_temp_dir("sandbox-cwd-initial");
        let target_dir = unique_temp_dir("sandbox-cwd-target");
        tokio::fs::create_dir_all(&initial_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();
        let app_state = Arc::new(Mutex::new(AppState::new(initial_dir.clone())));
        {
            let mut state = app_state.lock().await;
            state.config.sandbox.enabled = true;
            state.config.sandbox.allowed_paths = vec![initial_dir.clone(), target_dir.clone()];
        }

        let result = BashTool::new()
            .execute(
                serde_json::json!({ "command": format!("cd {} && pwd", target_dir.display()) }),
                context_with_state(app_state.clone()),
            )
            .await
            .unwrap();

        let target_dir = target_dir.canonicalize().unwrap();
        if result.is_error {
            assert!(!result.content.is_empty());
            return;
        }
        assert_eq!(app_state.lock().await.cwd, target_dir);
    }
}
