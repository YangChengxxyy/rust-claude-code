use std::path::PathBuf;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use tokio::fs;

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct FileWriteTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct FileWriteInput {
    path: PathBuf,
    content: String,
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "FileWrite".to_string(),
            description: "Create or overwrite a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: FileWriteInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        // Check file state cache for staleness/partial-view
        if let Some(app_state) = &context.app_state {
            let mut state = app_state.lock().await;
            // Check partial view first
            if let Some(file_state) = state.file_state_cache.get_read_state(&input.path) {
                if file_state.is_partial_view {
                    return Ok(ToolResult::error(
                        context.tool_use_id,
                        "File was read as partial view (system-injected). Please read the file with FileRead before writing.".to_string(),
                    ));
                }
            }
            // Check staleness
            if let Some(true) = state.file_state_cache.is_stale(&input.path) {
                return Ok(ToolResult::error(
                    context.tool_use_id,
                    "File has been modified since last read. Please re-read the file before writing.".to_string(),
                ));
            }
        }

        if let Some(parent) = input.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| ToolError::Execution(error.to_string()))?;
        }

        fs::write(&input.path, &input.content)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        // Record the write in cache
        if let Some(app_state) = &context.app_state {
            let mut state = app_state.lock().await;
            state
                .file_state_cache
                .record_write(&input.path, &input.content);
        }

        Ok(ToolResult::success(
            context.tool_use_id,
            format!("Wrote {}", input.path.display()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_write_rejects_stale_file() {
        use rust_claude_core::state::AppState;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let base = std::env::temp_dir().join(format!("write-stale-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("stale.txt");
        fs::write(&path, "original").await.unwrap();

        let state = Arc::new(Mutex::new(AppState::new(base.clone())));

        // Record a read
        {
            let mut s = state.lock().await;
            s.file_state_cache
                .record_read(&path, "original", None, None, false);
        }

        // Externally modify
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        fs::write(&path, "externally changed").await.unwrap();

        let result = FileWriteTool::new()
            .execute(
                serde_json::json!({ "path": path, "content": "new content" }),
                ToolContext {
                    tool_use_id: "tool_stale".to_string(),
                    app_state: Some(state),
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("modified since last read"));
    }

    #[tokio::test]
    async fn test_file_write_allows_unknown_file() {
        use rust_claude_core::state::AppState;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let base = std::env::temp_dir().join(format!("write-unknown-{}", std::process::id()));
        let path = base.join("new-file.txt");

        let state = Arc::new(Mutex::new(AppState::new(base.clone())));

        // No prior read — should be allowed
        let result = FileWriteTool::new()
            .execute(
                serde_json::json!({ "path": path, "content": "hello" }),
                ToolContext {
                    tool_use_id: "tool_unknown".to_string(),
                    app_state: Some(state),
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hello");
    }

    #[tokio::test]
    async fn test_file_write_creates_parent_dirs() {
        let base = std::env::temp_dir().join(format!("write-tool-{}", std::process::id()));
        let path = base.join("nested/out.txt");

        let result = FileWriteTool::new()
            .execute(
                serde_json::json!({ "path": path, "content": "hello" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let content = fs::read_to_string(base.join("nested/out.txt"))
            .await
            .unwrap();
        assert_eq!(content, "hello");
    }
}
