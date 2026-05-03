use std::path::PathBuf;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use tokio::fs;

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct FileEditTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct FileEditInput {
    path: PathBuf,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

impl FileEditTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "FileEdit".to_string(),
            description: "Edit an existing file by replacing text".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old_string": { "type": "string" },
                    "new_string": { "type": "string" },
                    "replace_all": { "type": "boolean" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: FileEditInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        // Check file state cache for staleness/partial-view (only for existing files)
        if !input.old_string.is_empty() {
            if let Some(app_state) = &context.app_state {
                let mut state = app_state.lock().await;
                // Check partial view first
                if let Some(file_state) = state.file_state_cache.get_read_state(&input.path) {
                    if file_state.is_partial_view {
                        return Ok(ToolResult::error(
                            context.tool_use_id,
                            "File was read as partial view (system-injected). Please read the file with FileRead before editing.".to_string(),
                        ));
                    }
                }
                // Check staleness
                if let Some(true) = state.file_state_cache.is_stale(&input.path) {
                    return Ok(ToolResult::error(
                        context.tool_use_id,
                        "File has been modified since last read. Please re-read the file before editing.".to_string(),
                    ));
                }
            }
        }

        if input.old_string.is_empty() {
            if let Some(parent) = input.path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|error| ToolError::Execution(error.to_string()))?;
            }
            fs::write(&input.path, &input.new_string)
                .await
                .map_err(|error| ToolError::Execution(error.to_string()))?;

            // Record the write in cache
            if let Some(app_state) = &context.app_state {
                let mut state = app_state.lock().await;
                state
                    .file_state_cache
                    .record_write(&input.path, &input.new_string);
            }

            return Ok(ToolResult::success(
                context.tool_use_id,
                format!("Created {}", input.path.display()),
            ));
        }

        let content = fs::read_to_string(&input.path)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let count = content.matches(&input.old_string).count();
        if count == 0 {
            return Err(ToolError::Execution("old_string not found".to_string()));
        }
        if count > 1 && !input.replace_all {
            return Err(ToolError::Execution(
                "old_string matched multiple times".to_string(),
            ));
        }

        let updated = if input.replace_all {
            content.replace(&input.old_string, &input.new_string)
        } else {
            content.replacen(&input.old_string, &input.new_string, 1)
        };

        fs::write(&input.path, &updated)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        // Record the write in cache
        if let Some(app_state) = &context.app_state {
            let mut state = app_state.lock().await;
            state.file_state_cache.record_write(&input.path, &updated);
        }

        Ok(ToolResult::success(
            context.tool_use_id,
            format!("Edited {}", input.path.display()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_edit_single_replace() {
        let base = std::env::temp_dir().join(format!("edit-tool-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("sample.txt");
        fs::write(&path, "hello world").await.unwrap();

        FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "world", "new_string": "rust" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        let content = fs::read_to_string(base.join("sample.txt")).await.unwrap();
        assert_eq!(content, "hello rust");
    }

    #[tokio::test]
    async fn test_file_edit_replace_all() {
        let base = std::env::temp_dir().join(format!("edit-all-tool-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("sample.txt");
        fs::write(&path, "a b a").await.unwrap();

        FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "a", "new_string": "x", "replace_all": true }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        let content = fs::read_to_string(base.join("sample.txt")).await.unwrap();
        assert_eq!(content, "x b x");
    }

    #[tokio::test]
    async fn test_file_edit_rejects_non_unique_match_without_replace_all() {
        let base = std::env::temp_dir().join(format!("edit-unique-tool-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("sample.txt");
        fs::write(&path, "a b a").await.unwrap();

        let error = FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "a", "new_string": "x" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap_err();

        assert!(
            matches!(error, ToolError::Execution(message) if message.contains("multiple times"))
        );
    }

    #[tokio::test]
    async fn test_file_edit_rejects_stale_file() {
        use rust_claude_core::state::AppState;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let base = std::env::temp_dir().join(format!("edit-stale-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("stale.txt");
        fs::write(&path, "original content").await.unwrap();

        let state = Arc::new(Mutex::new(AppState::new(base.clone())));

        // Record a read
        {
            let mut s = state.lock().await;
            s.file_state_cache
                .record_read(&path, "original content", None, None, false);
        }

        // Externally modify the file
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        fs::write(&path, "modified externally").await.unwrap();

        let result = FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "original", "new_string": "new" }),
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
    async fn test_file_edit_rejects_partial_view() {
        use rust_claude_core::state::AppState;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let base = std::env::temp_dir().join(format!("edit-partial-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("partial.txt");
        fs::write(&path, "file content").await.unwrap();

        let state = Arc::new(Mutex::new(AppState::new(base.clone())));

        // Record as partial view (system-injected)
        {
            let mut s = state.lock().await;
            s.file_state_cache
                .record_read(&path, "file content", None, None, true);
        }

        let result = FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "file", "new_string": "new" }),
                ToolContext {
                    tool_use_id: "tool_partial".to_string(),
                    app_state: Some(state),
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("partial view"));
    }

    #[tokio::test]
    async fn test_file_edit_allows_non_stale_file() {
        use rust_claude_core::state::AppState;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let base = std::env::temp_dir().join(format!("edit-fresh-{}", std::process::id()));
        fs::create_dir_all(&base).await.unwrap();
        let path = base.join("fresh.txt");
        fs::write(&path, "hello world").await.unwrap();

        let state = Arc::new(Mutex::new(AppState::new(base.clone())));

        // Record a read
        {
            let mut s = state.lock().await;
            s.file_state_cache
                .record_read(&path, "hello world", None, None, false);
        }

        let result = FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "world", "new_string": "rust" }),
                ToolContext {
                    tool_use_id: "tool_fresh".to_string(),
                    app_state: Some(state.clone()),
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);

        // Verify cache was updated after edit
        let s = state.lock().await;
        let cached = s.file_state_cache.get_read_state(&path);
        assert!(cached.is_some());
        assert!(!cached.unwrap().is_partial_view);
    }

    #[tokio::test]
    async fn test_file_edit_creates_new_file_when_old_string_empty() {
        let base = std::env::temp_dir().join(format!("edit-create-tool-{}", std::process::id()));
        let path = base.join("nested/new.txt");

        FileEditTool::new()
            .execute(
                serde_json::json!({ "path": path, "old_string": "", "new_string": "hello" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        let content = fs::read_to_string(base.join("nested/new.txt"))
            .await
            .unwrap();
        assert_eq!(content, "hello");
    }
}
