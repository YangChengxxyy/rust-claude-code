use std::path::PathBuf;
use std::time::Instant;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::tool::{Tool, ToolContext, ToolError};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_OUTPUT_LEN: usize = 8_000;

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

        let head_len = MAX_OUTPUT_LEN / 2;
        let tail_len = MAX_OUTPUT_LEN - head_len;
        let head = &output[..head_len];
        let tail = &output[output.len() - tail_len..];

        (
            format!("{head}\n... output truncated ...\n{tail}"),
            true,
        )
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
        true
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
        let mut command = Command::new("sh");
        command.arg("-c").arg(&input.command);
        if let Some(workdir) = &input.workdir {
            command.current_dir(workdir);
        }

        let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let output = timeout(Duration::from_millis(timeout_ms), command.output())
            .await
            .map_err(|_| ToolError::Execution("command timed out".to_string()))?
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        let duration_ms = started_at.elapsed().as_millis() as u64;
        let combined_output = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
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

    #[tokio::test]
    async fn test_bash_executes_simple_command() {
        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "command": "printf hello" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
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
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::Execution(message) if message.contains("timed out")));
    }

    #[tokio::test]
    async fn test_bash_respects_workdir() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rust-claude-tools-{}",
            std::process::id()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();
        let file_path = temp_dir.join("sample.txt");
        tokio::fs::write(&file_path, "hello").await.unwrap();

        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "command": "ls", "workdir": temp_dir }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(result.content.contains("sample.txt"));
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
}
