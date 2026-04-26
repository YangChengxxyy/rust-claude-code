use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rust_claude_core::hooks::{
    BaseHookInput, HookCommandResponse, HookConfig, HookEvent, HookResult, HooksConfig,
    PostToolUseInput, PreToolUseInput, StopInput, UserPromptSubmitInput,
};
use tokio::io::AsyncWriteExt;

const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Executes hooks for lifecycle events.
pub struct HookRunner {
    hooks: HooksConfig,
    cwd: PathBuf,
}

impl HookRunner {
    pub fn new(hooks: HooksConfig, cwd: PathBuf) -> Self {
        Self { hooks, cwd }
    }

    /// Returns `true` if no hooks are configured at all.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Returns a reference to the raw hooks configuration (for display).
    pub fn config(&self) -> &HooksConfig {
        &self.hooks
    }

    // -----------------------------------------------------------------------
    // Public API: event-specific entry points
    // -----------------------------------------------------------------------

    pub async fn run_pre_tool_use(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        session_id: &str,
    ) -> HookResult {
        self.run_pre_tool_use_with_cwd(tool_name, tool_input, session_id, &self.cwd)
            .await
    }

    pub async fn run_pre_tool_use_with_cwd(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        session_id: &str,
        cwd: &Path,
    ) -> HookResult {
        let matching = self.get_matching_hooks(&HookEvent::PreToolUse, Some(tool_name));
        if matching.is_empty() {
            return HookResult::Continue;
        }

        let input_json = serde_json::to_string(&PreToolUseInput {
            base: BaseHookInput {
                session_id: session_id.to_string(),
                cwd: cwd.to_string_lossy().to_string(),
            },
            tool_name: tool_name.to_string(),
            tool_input: tool_input.clone(),
        })
        .unwrap_or_default();

        for config in &matching {
            let result = self
                .execute_and_parse_pre_tool_use(config, &input_json, cwd)
                .await;
            if let HookResult::Block { .. } = &result {
                return result;
            }
        }

        HookResult::Continue
    }

    pub async fn run_post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        tool_output: &str,
        is_error: bool,
        session_id: &str,
    ) {
        self.run_post_tool_use_with_cwd(
            tool_name,
            tool_input,
            tool_output,
            is_error,
            session_id,
            &self.cwd,
        )
        .await;
    }

    pub async fn run_post_tool_use_with_cwd(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        tool_output: &str,
        is_error: bool,
        session_id: &str,
        cwd: &Path,
    ) {
        let matching = self.get_matching_hooks(&HookEvent::PostToolUse, Some(tool_name));
        if matching.is_empty() {
            return;
        }

        let input_json = serde_json::to_string(&PostToolUseInput {
            base: BaseHookInput {
                session_id: session_id.to_string(),
                cwd: cwd.to_string_lossy().to_string(),
            },
            tool_name: tool_name.to_string(),
            tool_input: tool_input.clone(),
            tool_output: tool_output.to_string(),
            tool_is_error: is_error,
        })
        .unwrap_or_default();

        for config in &matching {
            let _ = self
                .execute_command_hook(config, &input_json, &HookEvent::PostToolUse, cwd)
                .await;
        }
    }

    pub async fn run_user_prompt_submit(&self, user_message: &str, session_id: &str) {
        let matching = self.get_matching_hooks(&HookEvent::UserPromptSubmit, None);
        if matching.is_empty() {
            return;
        }

        let input_json = serde_json::to_string(&UserPromptSubmitInput {
            base: BaseHookInput {
                session_id: session_id.to_string(),
                cwd: self.cwd.to_string_lossy().to_string(),
            },
            user_message: user_message.to_string(),
        })
        .unwrap_or_default();

        for config in &matching {
            let _ = self
                .execute_command_hook(config, &input_json, &HookEvent::UserPromptSubmit, &self.cwd)
                .await;
        }
    }

    pub async fn run_stop(&self, stop_reason: &str, session_id: &str) {
        let matching = self.get_matching_hooks(&HookEvent::Stop, None);
        if matching.is_empty() {
            return;
        }

        let input_json = serde_json::to_string(&StopInput {
            base: BaseHookInput {
                session_id: session_id.to_string(),
                cwd: self.cwd.to_string_lossy().to_string(),
            },
            stop_reason: stop_reason.to_string(),
        })
        .unwrap_or_default();

        for config in &matching {
            let _ = self
                .execute_command_hook(config, &input_json, &HookEvent::Stop, &self.cwd)
                .await;
        }
    }

    // -----------------------------------------------------------------------
    // Internal: matching & execution
    // -----------------------------------------------------------------------

    /// Collect all `HookConfig` entries that match the given event and optional tool name.
    /// Returns configs in configuration order (user hooks first, project hooks after).
    fn get_matching_hooks(&self, event: &HookEvent, tool_name: Option<&str>) -> Vec<&HookConfig> {
        let event_key = event.as_str();
        let groups = match self.hooks.get(event_key) {
            Some(groups) => groups,
            None => return Vec::new(),
        };

        let is_tool_event = matches!(event, HookEvent::PreToolUse | HookEvent::PostToolUse);

        let mut result = Vec::new();
        for group in groups {
            // For tool events, check the matcher against tool_name.
            if is_tool_event {
                if let Some(matcher) = &group.matcher {
                    if !matcher.is_empty() {
                        if let Some(tool) = tool_name {
                            if matcher != tool {
                                continue;
                            }
                        }
                    }
                }
                // matcher is None or empty → matches all tools
            }
            // For non-tool events, matcher is ignored.

            for hook in &group.hooks {
                if hook.type_ != "command" {
                    eprintln!("Warning: unsupported hook type '{}', skipping", hook.type_);
                    continue;
                }
                if hook.command.is_none() {
                    continue;
                }
                result.push(hook);
            }
        }

        result
    }

    /// Execute a single command hook and return (stdout, exit_code).
    async fn execute_command_hook(
        &self,
        config: &HookConfig,
        input_json: &str,
        event: &HookEvent,
        cwd: &Path,
    ) -> Option<(String, i32)> {
        let command = config.command.as_deref()?;
        let timeout_secs = config.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());

        let mut child = match tokio::process::Command::new(&shell)
            .arg("-c")
            .arg(command)
            .env("CLAUDE_PROJECT_DIR", cwd)
            .env("HOOK_EVENT", event.as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                eprintln!("Warning: failed to spawn hook command '{}': {}", command, e);
                return None;
            }
        };

        // Write input JSON to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input_json.as_bytes()).await;
            drop(stdin);
        }

        // Wait with timeout.
        // `wait_with_output` consumes `child`. On timeout the future is dropped,
        // and `kill_on_drop(true)` (set on the Command above) ensures the child
        // process is killed instead of becoming a zombie.
        let timeout_dur = Duration::from_secs(timeout_secs);
        match tokio::time::timeout(timeout_dur, child.wait_with_output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                Some((stdout, exit_code))
            }
            Ok(Err(e)) => {
                eprintln!("Warning: hook command '{}' failed: {}", command, e);
                None
            }
            Err(_) => {
                // Timeout — child is dropped here; kill_on_drop ensures cleanup.
                eprintln!(
                    "Warning: hook command '{}' timed out after {}s",
                    command, timeout_secs
                );
                None
            }
        }
    }

    /// Execute a PreToolUse hook and parse its result.
    async fn execute_and_parse_pre_tool_use(
        &self,
        config: &HookConfig,
        input_json: &str,
        cwd: &Path,
    ) -> HookResult {
        let (stdout, exit_code) = match self
            .execute_command_hook(config, input_json, &HookEvent::PreToolUse, cwd)
            .await
        {
            Some(result) => result,
            None => return HookResult::Continue, // spawn failed or timeout → approve
        };

        Self::parse_pre_tool_use_result(&stdout, exit_code)
    }

    /// Parse the result of a PreToolUse hook execution.
    /// Exit codes: 0 = approve, 1 = warn (non-blocking), 2 = block.
    /// If stdout is valid JSON with `decision: "block"`, that takes precedence.
    pub fn parse_pre_tool_use_result(stdout: &str, exit_code: i32) -> HookResult {
        // Exit code 2 → blocking error
        if exit_code == 2 {
            let reason = if stdout.trim().is_empty() {
                "Hook exited with code 2".to_string()
            } else {
                // Try to parse JSON for a reason
                if let Ok(resp) = serde_json::from_str::<HookCommandResponse>(stdout) {
                    resp.reason
                        .unwrap_or_else(|| "Hook exited with code 2".to_string())
                } else {
                    stdout.trim().to_string()
                }
            };
            return HookResult::Block { reason };
        }

        // Exit code 1 → warning (non-blocking)
        if exit_code == 1 {
            if !stdout.trim().is_empty() {
                eprintln!("Warning: hook: {}", stdout.trim());
            }
            return HookResult::Continue;
        }

        // Exit code 0 or other: try to parse JSON
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return HookResult::Continue;
        }

        match serde_json::from_str::<HookCommandResponse>(trimmed) {
            Ok(resp) => {
                if resp.decision.as_deref() == Some("block") {
                    HookResult::Block {
                        reason: resp.reason.unwrap_or_else(|| "Blocked by hook".to_string()),
                    }
                } else {
                    HookResult::Continue
                }
            }
            Err(_) => {
                // Invalid JSON → approve
                eprintln!("Warning: hook stdout is not valid JSON, treating as approve");
                HookResult::Continue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::hooks::HookEventGroup;
    use std::collections::HashMap;

    fn make_hooks_config(event: &str, matcher: Option<&str>, command: &str) -> HooksConfig {
        let mut config = HashMap::new();
        config.insert(
            event.to_string(),
            vec![HookEventGroup {
                matcher: matcher.map(String::from),
                hooks: vec![HookConfig {
                    type_: "command".to_string(),
                    command: Some(command.to_string()),
                    timeout: None,
                }],
            }],
        );
        config
    }

    // -----------------------------------------------------------------------
    // Matcher filtering tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_matcher_matches_tool_name() {
        let hooks = make_hooks_config("PreToolUse", Some("Bash"), "check.sh");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::PreToolUse, Some("Bash"));
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_matcher_does_not_match_different_tool() {
        let hooks = make_hooks_config("PreToolUse", Some("Bash"), "check.sh");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::PreToolUse, Some("FileRead"));
        assert_eq!(matched.len(), 0);
    }

    #[test]
    fn test_empty_matcher_matches_all() {
        let hooks = make_hooks_config("PreToolUse", Some(""), "check.sh");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::PreToolUse, Some("Bash"));
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_no_matcher_matches_all() {
        let hooks = make_hooks_config("PreToolUse", None, "check.sh");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::PreToolUse, Some("FileWrite"));
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_matcher_ignored_for_non_tool_events() {
        let hooks = make_hooks_config("UserPromptSubmit", Some("Bash"), "log.sh");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::UserPromptSubmit, None);
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_unsupported_type_skipped() {
        let mut config = HashMap::new();
        config.insert(
            "PreToolUse".to_string(),
            vec![HookEventGroup {
                matcher: None,
                hooks: vec![HookConfig {
                    type_: "prompt".to_string(),
                    command: Some("check $ARGUMENTS".to_string()),
                    timeout: None,
                }],
            }],
        );
        let runner = HookRunner::new(config, PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::PreToolUse, Some("Bash"));
        assert_eq!(matched.len(), 0);
    }

    #[test]
    fn test_no_event_configured() {
        let runner = HookRunner::new(HashMap::new(), PathBuf::from("/tmp"));
        let matched = runner.get_matching_hooks(&HookEvent::PreToolUse, Some("Bash"));
        assert_eq!(matched.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Result parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_approve_json() {
        let result = HookRunner::parse_pre_tool_use_result(r#"{"decision": "approve"}"#, 0);
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_parse_block_json() {
        let result = HookRunner::parse_pre_tool_use_result(
            r#"{"decision": "block", "reason": "unsafe command"}"#,
            0,
        );
        assert_eq!(
            result,
            HookResult::Block {
                reason: "unsafe command".into()
            }
        );
    }

    #[test]
    fn test_parse_block_without_reason() {
        let result = HookRunner::parse_pre_tool_use_result(r#"{"decision": "block"}"#, 0);
        assert_eq!(
            result,
            HookResult::Block {
                reason: "Blocked by hook".into()
            }
        );
    }

    #[test]
    fn test_parse_empty_stdout() {
        let result = HookRunner::parse_pre_tool_use_result("", 0);
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = HookRunner::parse_pre_tool_use_result("not json", 0);
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_parse_empty_json() {
        let result = HookRunner::parse_pre_tool_use_result("{}", 0);
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_parse_exit_code_2_blocks() {
        let result = HookRunner::parse_pre_tool_use_result("", 2);
        assert_eq!(
            result,
            HookResult::Block {
                reason: "Hook exited with code 2".into()
            }
        );
    }

    #[test]
    fn test_parse_exit_code_2_with_json_reason() {
        let result = HookRunner::parse_pre_tool_use_result(r#"{"reason": "policy violation"}"#, 2);
        assert_eq!(
            result,
            HookResult::Block {
                reason: "policy violation".into()
            }
        );
    }

    #[test]
    fn test_parse_exit_code_2_with_plain_text() {
        let result = HookRunner::parse_pre_tool_use_result("forbidden\n", 2);
        assert_eq!(
            result,
            HookResult::Block {
                reason: "forbidden".into()
            }
        );
    }

    #[test]
    fn test_parse_exit_code_1_continues() {
        let result = HookRunner::parse_pre_tool_use_result("some warning", 1);
        assert_eq!(result, HookResult::Continue);
    }

    // -----------------------------------------------------------------------
    // Integration tests (actually spawn processes)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_run_pre_tool_use_approve() {
        let hooks = make_hooks_config("PreToolUse", Some("Bash"), "echo '{}'");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let result = runner
            .run_pre_tool_use("Bash", &serde_json::json!({"command": "ls"}), "")
            .await;
        assert_eq!(result, HookResult::Continue);
    }

    #[tokio::test]
    async fn test_run_pre_tool_use_block() {
        let hooks = make_hooks_config(
            "PreToolUse",
            Some("Bash"),
            r#"echo '{"decision":"block","reason":"nope"}'"#,
        );
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let result = runner
            .run_pre_tool_use("Bash", &serde_json::json!({"command": "rm -rf /"}), "")
            .await;
        assert_eq!(
            result,
            HookResult::Block {
                reason: "nope".into()
            }
        );
    }

    #[tokio::test]
    async fn test_run_pre_tool_use_no_matching_hooks() {
        let hooks = make_hooks_config("PreToolUse", Some("Bash"), "echo block");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let result = runner
            .run_pre_tool_use("FileRead", &serde_json::json!({}), "")
            .await;
        assert_eq!(result, HookResult::Continue);
    }

    #[tokio::test]
    async fn test_run_pre_tool_use_exit_code_2() {
        let hooks = make_hooks_config("PreToolUse", Some("Bash"), "exit 2");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let result = runner
            .run_pre_tool_use("Bash", &serde_json::json!({}), "")
            .await;
        assert!(matches!(result, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_run_pre_tool_use_timeout() {
        let mut hooks = make_hooks_config("PreToolUse", Some("Bash"), "sleep 30");
        // Set a very short timeout
        if let Some(groups) = hooks.get_mut("PreToolUse") {
            groups[0].hooks[0].timeout = Some(1);
        }
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        let result = runner
            .run_pre_tool_use("Bash", &serde_json::json!({}), "")
            .await;
        // Timeout → approve (non-blocking)
        assert_eq!(result, HookResult::Continue);
    }

    #[tokio::test]
    async fn test_run_pre_tool_use_short_circuits_on_block() {
        // First hook blocks, second should not run
        let mut config: HooksConfig = HashMap::new();
        config.insert(
            "PreToolUse".to_string(),
            vec![
                HookEventGroup {
                    matcher: None,
                    hooks: vec![HookConfig {
                        type_: "command".to_string(),
                        command: Some(
                            r#"echo '{"decision":"block","reason":"first"}'"#.to_string(),
                        ),
                        timeout: None,
                    }],
                },
                HookEventGroup {
                    matcher: None,
                    hooks: vec![HookConfig {
                        type_: "command".to_string(),
                        // This would create a file if it ran — we check it doesn't
                        command: Some(
                            "touch /tmp/rust-claude-hook-test-should-not-exist".to_string(),
                        ),
                        timeout: None,
                    }],
                },
            ],
        );
        let runner = HookRunner::new(config, PathBuf::from("/tmp"));
        let result = runner
            .run_pre_tool_use("Bash", &serde_json::json!({}), "")
            .await;
        assert_eq!(
            result,
            HookResult::Block {
                reason: "first".into()
            }
        );
        // Verify second hook didn't run
        assert!(!std::path::Path::new("/tmp/rust-claude-hook-test-should-not-exist").exists());
    }

    #[tokio::test]
    async fn test_run_post_tool_use() {
        // Just verify it doesn't panic and runs the hook
        let hooks = make_hooks_config("PostToolUse", Some("Bash"), "cat > /dev/null");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        runner
            .run_post_tool_use(
                "Bash",
                &serde_json::json!({"command": "ls"}),
                "output",
                false,
                "",
            )
            .await;
    }

    #[tokio::test]
    async fn test_run_user_prompt_submit() {
        let hooks = make_hooks_config("UserPromptSubmit", None, "cat > /dev/null");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        runner.run_user_prompt_submit("hello", "").await;
    }

    #[tokio::test]
    async fn test_run_stop() {
        let hooks = make_hooks_config("Stop", None, "cat > /dev/null");
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp"));
        runner.run_stop("end_turn", "").await;
    }

    #[tokio::test]
    async fn test_hook_receives_env_vars() {
        let hooks = make_hooks_config(
            "PreToolUse",
            None,
            r#"echo "{\"decision\":\"$HOOK_EVENT\"}""#,
        );
        let runner = HookRunner::new(hooks, PathBuf::from("/tmp/test-project"));
        // The hook outputs {"decision":"PreToolUse"} which is not "block", so it approves
        let result = runner
            .run_pre_tool_use("Bash", &serde_json::json!({}), "")
            .await;
        assert_eq!(result, HookResult::Continue);
    }
}
