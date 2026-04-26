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

        if let Some(parent) = input.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| ToolError::Execution(error.to_string()))?;
        }

        fs::write(&input.path, input.content)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;

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
