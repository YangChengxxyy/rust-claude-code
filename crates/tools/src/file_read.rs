use std::path::PathBuf;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use tokio::fs;

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct FileReadTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct FileReadInput {
    path: PathBuf,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

impl FileReadTool {
    pub fn new() -> Self {
        Self
    }

    async fn read_path(input: FileReadInput) -> Result<String, ToolError> {
        let metadata = fs::metadata(&input.path)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        if metadata.is_dir() {
            let mut entries = fs::read_dir(&input.path)
                .await
                .map_err(|error| ToolError::Execution(error.to_string()))?;
            let mut names = Vec::new();
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|error| ToolError::Execution(error.to_string()))?
            {
                let file_type = entry
                    .file_type()
                    .await
                    .map_err(|error| ToolError::Execution(error.to_string()))?;
                let mut name = entry.file_name().to_string_lossy().to_string();
                if file_type.is_dir() {
                    name.push('/');
                }
                names.push(name);
            }
            names.sort();
            return Ok(names.join("\n"));
        }

        let content = fs::read_to_string(&input.path)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let normalized = content.replace("\r\n", "\n");
        let mut lines: Vec<&str> = normalized.split('\n').collect();
        if normalized.ends_with('\n') {
            lines.pop();
        }
        let offset = input.offset.unwrap_or(0);
        let limit = input.limit.unwrap_or(lines.len());

        let rendered = lines
            .iter()
            .enumerate()
            .skip(offset)
            .take(limit)
            .map(|(index, line)| format!("{}: {}", index + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(rendered)
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "FileRead".to_string(),
            description: "Read file contents or list a directory".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "offset": { "type": "integer", "minimum": 0 },
                    "limit": { "type": "integer", "minimum": 1 }
                },
                "required": ["path"]
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
        let input: FileReadInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let path = input.path.clone();
        let offset = input.offset;
        let limit = input.limit;

        let content = Self::read_path(input).await?;

        // Record read into file state cache (if app_state is available).
        // Only record for file paths (not directories).
        if let Some(app_state) = &context.app_state {
            if path.is_file() {
                let mut state = app_state.lock().await;
                state
                    .file_state_cache
                    .record_read(&path, &content, offset, limit, false);
            }
        }

        Ok(ToolResult::success(context.tool_use_id, content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_read_reads_with_line_numbers() {
        let temp_dir = std::env::temp_dir().join(format!("read-tool-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).await.unwrap();
        let path = temp_dir.join("sample.txt");
        fs::write(&path, "a\nb\nc\n").await.unwrap();

        let result = FileReadTool::new()
            .execute(
                serde_json::json!({ "path": path, "offset": 1, "limit": 2 }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "2: b\n3: c");
    }

    #[tokio::test]
    async fn test_file_read_preserves_empty_lines() {
        let temp_dir = std::env::temp_dir().join(format!("read-empty-tool-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).await.unwrap();
        let path = temp_dir.join("sample.txt");
        fs::write(&path, "a\n\nb\n").await.unwrap();

        let result = FileReadTool::new()
            .execute(
                serde_json::json!({ "path": path }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "1: a\n2: \n3: b");
    }

    #[tokio::test]
    async fn test_file_read_lists_directory() {
        let temp_dir = std::env::temp_dir().join(format!("read-dir-tool-{}", std::process::id()));
        fs::create_dir_all(temp_dir.join("nested")).await.unwrap();
        fs::write(temp_dir.join("file.txt"), "x").await.unwrap();

        let result = FileReadTool::new()
            .execute(
                serde_json::json!({ "path": temp_dir }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(result.content.contains("file.txt"));
        assert!(result.content.contains("nested/"));
    }
}
