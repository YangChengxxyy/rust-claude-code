use std::path::PathBuf;
use std::time::SystemTime;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct GlobTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct GlobInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Glob".to_string(),
            description: "Fast file pattern matching tool that searches for files by glob patterns"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match files against"
                    },
                    "path": {
                        "type": "string",
                        "description": "The directory to search in. Defaults to current working directory."
                    }
                },
                "required": ["pattern"]
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
        let input: GlobInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        let search_root = match &input.path {
            Some(p) => PathBuf::from(p),
            None => std::env::current_dir()
                .map_err(|e| ToolError::Execution(format!("cannot get cwd: {e}")))?,
        };

        // Build the full glob pattern by joining root + pattern
        let full_pattern = search_root.join(&input.pattern);
        let full_pattern_str = full_pattern.to_string_lossy();

        let entries = glob::glob(&full_pattern_str)
            .map_err(|e| ToolError::InvalidInput(format!("invalid glob pattern: {e}")))?;

        // Collect matching paths with their modification times
        let mut matched: Vec<(PathBuf, SystemTime)> = Vec::new();
        for entry in entries {
            match entry {
                Ok(path) => {
                    let mtime = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    matched.push((path, mtime));
                }
                Err(_) => continue, // skip unreadable entries
            }
        }

        // Sort by modification time, newest first
        matched.sort_by(|a, b| b.1.cmp(&a.1));

        let output = matched
            .iter()
            .map(|(p, _)| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult::success(context.tool_use_id, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    async fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("glob-tool-{}-{}", std::process::id(), suffix));
        let _ = fs::remove_dir_all(&dir).await;
        fs::create_dir_all(&dir).await.unwrap();
        dir
    }

    fn ctx() -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            app_state: None,
            agent_context: None,
            user_question_callback: None,
        }
    }

    #[tokio::test]
    async fn test_glob_basic_pattern() {
        let dir = make_temp_dir("basic").await;
        fs::write(dir.join("foo.rs"), "fn main(){}").await.unwrap();
        fs::write(dir.join("bar.rs"), "fn bar(){}").await.unwrap();
        fs::write(dir.join("baz.txt"), "hello").await.unwrap();

        let tool = GlobTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "pattern": "*.rs", "path": dir.to_str().unwrap() }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_glob_recursive_pattern() {
        let dir = make_temp_dir("recursive").await;
        fs::create_dir_all(dir.join("sub")).await.unwrap();
        fs::write(dir.join("a.rs"), "").await.unwrap();
        fs::write(dir.join("sub/b.rs"), "").await.unwrap();

        let tool = GlobTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "pattern": "**/*.rs", "path": dir.to_str().unwrap() }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = make_temp_dir("empty").await;
        fs::write(dir.join("foo.txt"), "hello").await.unwrap();

        let tool = GlobTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "pattern": "*.rs", "path": dir.to_str().unwrap() }),
                ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "");
    }

    #[tokio::test]
    async fn test_glob_mtime_sorting() {
        let dir = make_temp_dir("mtime").await;
        fs::write(dir.join("old.rs"), "old").await.unwrap();

        // Small delay to ensure different mtime
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        fs::write(dir.join("new.rs"), "new").await.unwrap();

        let tool = GlobTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "pattern": "*.rs", "path": dir.to_str().unwrap() }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("new.rs"), "newest file should be first");
        assert!(lines[1].contains("old.rs"), "oldest file should be second");
    }

    #[tokio::test]
    async fn test_glob_missing_pattern() {
        let tool = GlobTool::new();
        let result = tool.execute(serde_json::json!({}), ctx()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidInput(_) => {}
            other => panic!("expected InvalidInput, got: {other}"),
        }
    }
}
