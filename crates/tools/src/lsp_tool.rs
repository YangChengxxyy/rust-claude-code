use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::lsp::{LspManager, LspRequest};
use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspToolInput {
    operation: String,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    character: Option<u32>,
    #[serde(default)]
    query: Option<String>,
}

#[derive(Clone)]
pub struct LspTool {
    manager: Arc<LspManager>,
}

impl LspTool {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(LspManager::new()),
        }
    }

    fn file_uri(path: &std::path::Path) -> String {
        format!("file://{}", path.display())
    }

    fn require_position(input: &LspToolInput) -> Result<(u32, u32), ToolError> {
        Ok((
            input
                .line
                .ok_or_else(|| ToolError::InvalidInput("missing line".to_string()))?,
            input
                .character
                .ok_or_else(|| ToolError::InvalidInput("missing character".to_string()))?,
        ))
    }

    fn format_locations(value: serde_json::Value) -> Result<String, ToolError> {
        let locations = if value.is_array() {
            value
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|item| {
                    let uri = item.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                    let start = item
                        .get("range")
                        .and_then(|v| v.get("start"))
                        .cloned()
                        .unwrap_or_default();
                    let line = start.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                    let character = start
                        .get("character")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    format!("{}:{}:{}", uri, line, character)
                })
                .collect::<Vec<_>>()
        } else if value.is_object() {
            vec![serde_json::to_string_pretty(&value)
                .map_err(|e| ToolError::Execution(e.to_string()))?]
        } else {
            vec![value.to_string()]
        };
        Ok(locations.join("\n"))
    }
}

#[async_trait]
impl Tool for LspTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Lsp".to_string(),
            description: "Semantic code navigation via language servers: goToDefinition, findReferences, hover, documentSymbol, workspaceSymbol".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["goToDefinition", "findReferences", "hover", "documentSymbol", "workspaceSymbol"] },
                    "path": { "type": "string" },
                    "line": { "type": "integer", "minimum": 0 },
                    "character": { "type": "integer", "minimum": 0 },
                    "query": { "type": "string" }
                },
                "required": ["operation"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: LspToolInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let cwd = if let Some(state) = context.app_state.as_ref() {
            let state = state.lock().await;
            state.cwd.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        let output = match input.operation.as_str() {
            "goToDefinition" => {
                let path = input
                    .path
                    .as_ref()
                    .ok_or_else(|| ToolError::InvalidInput("missing path".to_string()))?;
                let (line, character) = Self::require_position(&input)?;
                let key = self.manager
                    .ensure_session(&cwd, path)
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let value = self.manager
                    .request(&key, LspRequest::go_to_definition(&Self::file_uri(path), line, character))
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                Self::format_locations(value)?
            }
            "findReferences" => {
                let path = input
                    .path
                    .as_ref()
                    .ok_or_else(|| ToolError::InvalidInput("missing path".to_string()))?;
                let (line, character) = Self::require_position(&input)?;
                let key = self.manager
                    .ensure_session(&cwd, path)
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let value = self.manager
                    .request(&key, LspRequest::find_references(&Self::file_uri(path), line, character))
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                Self::format_locations(value)?
            }
            "hover" => {
                let path = input
                    .path
                    .as_ref()
                    .ok_or_else(|| ToolError::InvalidInput("missing path".to_string()))?;
                let (line, character) = Self::require_position(&input)?;
                let key = self.manager
                    .ensure_session(&cwd, path)
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let value = self.manager
                    .request(&key, LspRequest::hover(&Self::file_uri(path), line, character))
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                serde_json::to_string_pretty(&value)
                    .map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "documentSymbol" => {
                let path = input
                    .path
                    .as_ref()
                    .ok_or_else(|| ToolError::InvalidInput("missing path".to_string()))?;
                let key = self.manager
                    .ensure_session(&cwd, path)
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let value = self.manager
                    .request(&key, LspRequest::document_symbol(&Self::file_uri(path)))
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                serde_json::to_string_pretty(&value)
                    .map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "workspaceSymbol" => {
                let query = input
                    .query
                    .as_deref()
                    .ok_or_else(|| ToolError::InvalidInput("missing query".to_string()))?;
                let any_path = input
                    .path
                    .as_ref()
                    .ok_or_else(|| ToolError::InvalidInput("missing path for language selection".to_string()))?;
                let key = self.manager
                    .ensure_session(&cwd, any_path)
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                let value = self.manager
                    .request(&key, LspRequest::workspace_symbol(query))
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                serde_json::to_string_pretty(&value)
                    .map_err(|e| ToolError::Execution(e.to_string()))?
            }
            other => {
                return Err(ToolError::InvalidInput(format!(
                    "unsupported LSP operation: {}",
                    other
                )))
            }
        };

        Ok(ToolResult::success(context.tool_use_id, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_lists_operations() {
        let schema = LspTool::new().info().input_schema;
        let ops = schema["properties"]["operation"]["enum"]
            .as_array()
            .unwrap();
        assert!(ops.iter().any(|v| v == "goToDefinition"));
        assert!(ops.iter().any(|v| v == "workspaceSymbol"));
    }
}
