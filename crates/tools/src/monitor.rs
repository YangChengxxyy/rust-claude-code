use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use regex::Regex;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use crate::tool::{Tool, ToolContext, ToolError};

const DEFAULT_CAPTURE_LIMIT: usize = 50;
const MAX_MATCHED_LINE_LEN: usize = 2_000;

#[derive(Debug, Clone, Default)]
pub struct MonitorTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct MonitorToolInput {
    command: String,
    pattern: String,
    timeout: u64,
    #[serde(default)]
    capture_limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct MonitorMatch {
    stream: &'static str,
    line: String,
    truncated: bool,
}

#[derive(Debug, Default, serde::Serialize)]
struct MonitorOutput {
    matches: Vec<MonitorMatch>,
    omitted_lines: usize,
    truncated_matching_lines: usize,
}

#[derive(Debug, serde::Serialize)]
struct MonitorReport {
    command: String,
    pattern: String,
    status: &'static str,
    exit_status: Option<String>,
    timed_out: bool,
    omitted_lines: usize,
    truncated_matching_lines: usize,
    matches: Vec<MonitorMatch>,
}

impl MonitorTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_input(input: serde_json::Value) -> Result<MonitorToolInput, ToolError> {
        let input: MonitorToolInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        if input.command.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "command cannot be empty".to_string(),
            ));
        }
        if input.pattern.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "pattern cannot be empty".to_string(),
            ));
        }
        if input.timeout == 0 {
            return Err(ToolError::InvalidInput(
                "timeout must be greater than zero".to_string(),
            ));
        }

        Ok(input)
    }

    fn timeout_duration(timeout_value: u64) -> Duration {
        Duration::from_millis(timeout_value)
    }

    fn truncate_line(line: String) -> (String, bool) {
        if line.len() <= MAX_MATCHED_LINE_LEN {
            return (line, false);
        }

        let mut end = MAX_MATCHED_LINE_LEN;
        while end > 0 && !line.is_char_boundary(end) {
            end -= 1;
        }
        (format!("{}... line truncated ...", &line[..end]), true)
    }

    fn spawn_reader<R>(
        reader: R,
        stream: &'static str,
        pattern: Arc<Regex>,
        output: Arc<Mutex<MonitorOutput>>,
        capture_limit: usize,
    ) -> tokio::task::JoinHandle<()>
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let mut output = output.lock().await;
                        if pattern.is_match(&line) {
                            if output.matches.len() < capture_limit {
                                let (line, truncated) = Self::truncate_line(line);
                                output.matches.push(MonitorMatch {
                                    stream,
                                    line,
                                    truncated,
                                });
                            } else {
                                output.truncated_matching_lines += 1;
                            }
                        } else {
                            output.omitted_lines += 1;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        })
    }

    async fn wait_for_child(
        child: &mut Child,
        timeout_duration: Duration,
    ) -> Result<(bool, Option<ExitStatus>), ToolError> {
        match timeout(timeout_duration, child.wait()).await {
            Ok(status) => status
                .map(|status| (false, Some(status)))
                .map_err(|error| ToolError::Execution(error.to_string())),
            Err(_) => {
                let _ = child.kill().await;
                let status = child.wait().await.ok();
                Ok((true, status))
            }
        }
    }

    fn format_report(
        input: MonitorToolInput,
        timed_out: bool,
        exit_status: Option<ExitStatus>,
        output: MonitorOutput,
    ) -> Result<String, ToolError> {
        let report = MonitorReport {
            command: input.command,
            pattern: input.pattern,
            status: if timed_out { "timeout" } else { "exited" },
            exit_status: exit_status.map(|status| status.to_string()),
            timed_out,
            omitted_lines: output.omitted_lines,
            truncated_matching_lines: output.truncated_matching_lines,
            matches: output.matches,
        };

        serde_json::to_string_pretty(&report)
            .map_err(|error| ToolError::Execution(error.to_string()))
    }
}

#[async_trait]
impl Tool for MonitorTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Monitor".to_string(),
            description: "Run a command and return stdout/stderr lines matching a regex"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "pattern": { "type": "string" },
                    "timeout": { "type": "integer", "minimum": 1, "description": "Timeout in milliseconds" },
                    "capture_limit": { "type": "integer", "minimum": 0 }
                },
                "required": ["command", "pattern", "timeout"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input = Self::validate_input(input)?;
        let pattern = Arc::new(
            Regex::new(&input.pattern)
                .map_err(|error| ToolError::InvalidInput(error.to_string()))?,
        );
        let capture_limit = input.capture_limit.unwrap_or(DEFAULT_CAPTURE_LIMIT);
        let timeout_duration = Self::timeout_duration(input.timeout);
        let started_at = Instant::now();

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&input.command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        let output = Arc::new(Mutex::new(MonitorOutput::default()));
        let mut reader_handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            reader_handles.push(Self::spawn_reader::<ChildStdout>(
                stdout,
                "stdout",
                pattern.clone(),
                output.clone(),
                capture_limit,
            ));
        }
        if let Some(stderr) = child.stderr.take() {
            reader_handles.push(Self::spawn_reader::<ChildStderr>(
                stderr,
                "stderr",
                pattern,
                output.clone(),
                capture_limit,
            ));
        }

        let (timed_out, exit_status) = Self::wait_for_child(&mut child, timeout_duration).await?;
        // Wait for reader tasks to finish draining all buffered output.
        for handle in reader_handles {
            let _ = handle.await;
        }
        let output = {
            let mut output = output.lock().await;
            std::mem::take(&mut *output)
        };
        let content = Self::format_report(input, timed_out, exit_status, output)?;
        let duration_ms = started_at.elapsed().as_millis() as u64;

        let result = if timed_out {
            ToolResult::error(context.tool_use_id, content)
        } else {
            ToolResult::success(context.tool_use_id, content)
        }
        .with_duration(duration_ms);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;

    fn context() -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            app_state: None,
            agent_context: None,
            user_question_callback: None,
        }
    }

    #[tokio::test]
    async fn test_monitor_returns_matched_output_and_omitted_count() {
        let tool = MonitorTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": "printf 'skip\\nmatch-one\\n'",
                    "pattern": "match",
                    "timeout": 1_000
                }),
                context(),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("match-one"));
        assert!(result.content.contains("\"stream\": \"stdout\""));
        assert!(result.content.contains("\"omitted_lines\": 1"));
        assert!(result.content.contains("\"status\": \"exited\""));
    }

    #[tokio::test]
    async fn test_monitor_reads_stderr() {
        let tool = MonitorTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": "printf 'err-match\\n' >&2",
                    "pattern": "err",
                    "timeout": 1_000
                }),
                context(),
            )
            .await
            .unwrap();

        assert!(result.content.contains("err-match"));
        assert!(result.content.contains("\"stream\": \"stderr\""));
    }

    #[tokio::test]
    async fn test_monitor_timeout_returns_collected_matches() {
        let tool = MonitorTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": "printf 'ready\\n'; sleep 1",
                    "pattern": "ready",
                    "timeout": 50
                }),
                context(),
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("\"status\": \"timeout\""));
        assert!(result.content.contains("ready"));
    }

    #[tokio::test]
    async fn test_monitor_truncates_by_capture_limit() {
        let tool = MonitorTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": "printf 'hit1\\nhit2\\nhit3\\n'",
                    "pattern": "hit",
                    "timeout": 1_000,
                    "capture_limit": 2
                }),
                context(),
            )
            .await
            .unwrap();

        let report: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let matches = report["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0]["line"], "hit1");
        assert_eq!(matches[1]["line"], "hit2");
        assert_eq!(report["truncated_matching_lines"], 1);
    }

    #[tokio::test]
    async fn test_monitor_truncates_long_matched_line() {
        let tool = MonitorTool::new();
        let long_line = "x".repeat(MAX_MATCHED_LINE_LEN + 100);
        let command = format!("printf '{}\\n'", long_line);
        let result = tool
            .execute(
                serde_json::json!({
                    "command": command,
                    "pattern": "x+",
                    "timeout": 1_000
                }),
                context(),
            )
            .await
            .unwrap();

        assert!(result.content.contains("line truncated"));
        assert!(result.content.contains("\"truncated\": true"));
    }
}
