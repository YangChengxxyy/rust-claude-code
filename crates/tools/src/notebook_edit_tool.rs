use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::tool::{Tool, ToolContext, ToolError};

pub struct NotebookEditTool;

impl NotebookEditTool {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct NotebookEditInput {
    file_path: String,
    operation: String,
    index: Option<usize>,
    cell_type: Option<String>,
    source: Option<String>,
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "NotebookEdit".to_string(),
            description: "Edit Jupyter notebook cells structurally".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string" },
                    "operation": { "type": "string", "enum": ["replace", "insert"] },
                    "index": { "type": "integer", "minimum": 0 },
                    "cell_type": { "type": "string", "enum": ["code", "markdown"] },
                    "source": { "type": "string" }
                },
                "required": ["file_path", "operation", "index"],
                "additionalProperties": false
            }),
        }
    }

    async fn execute(&self, input: Value, _context: ToolContext) -> Result<ToolResult, ToolError> {
        let input: NotebookEditInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid notebook edit input: {e}")))?;

        let path = PathBuf::from(&input.file_path);
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read notebook: {e}")))?;
        let mut notebook: Value = serde_json::from_str(&raw)
            .map_err(|e| ToolError::Execution(format!("file is not a valid notebook JSON document: {e}")))?;

        let cells = notebook
            .get_mut("cells")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| ToolError::Execution("notebook is missing top-level 'cells' array".to_string()))?;

        let index = input.index.ok_or_else(|| ToolError::InvalidInput("index is required".to_string()))?;
        let source_lines = input
            .source
            .unwrap_or_default()
            .lines()
            .map(|line| format!("{}\n", line))
            .collect::<Vec<_>>();

        match input.operation.as_str() {
            "replace" => {
                if index >= cells.len() {
                    return Err(ToolError::Execution(format!(
                        "cell index {index} is out of range for notebook with {} cells",
                        cells.len()
                    )));
                }
                let existing = cells
                    .get_mut(index)
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| ToolError::Execution("target cell is not a valid notebook cell object".to_string()))?;
                if let Some(cell_type) = input.cell_type {
                    existing.insert("cell_type".to_string(), Value::String(cell_type));
                }
                existing.insert(
                    "source".to_string(),
                    Value::Array(source_lines.into_iter().map(Value::String).collect()),
                );
            }
            "insert" => {
                if index > cells.len() {
                    return Err(ToolError::Execution(format!(
                        "cell index {index} is out of range for insert into notebook with {} cells",
                        cells.len()
                    )));
                }
                let cell_type = input.cell_type.unwrap_or_else(|| "code".to_string());
                let new_cell = json!({
                    "cell_type": cell_type,
                    "metadata": {},
                    "source": source_lines,
                    "outputs": [],
                    "execution_count": Value::Null,
                });
                cells.insert(index, new_cell);
            }
            other => {
                return Err(ToolError::InvalidInput(format!(
                    "unsupported operation '{other}', expected replace or insert"
                )));
            }
        }

        let formatted = serde_json::to_string_pretty(&notebook)
            .map_err(|e| ToolError::Execution(format!("failed to serialize notebook: {e}")))?;
        tokio::fs::write(&path, formatted)
            .await
            .map_err(|e| ToolError::Execution(format!("failed to write notebook: {e}")))?;

        Ok(ToolResult::success(
            _context.tool_use_id,
            format!(
                "Updated notebook {} with {} at cell index {}",
                path.display(),
                input.operation,
                index
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_dir(name: &str) -> std::path::PathBuf {
        let unique = format!("rust-claude-notebook-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_notebook(path: &std::path::Path) {
        let notebook = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "metadata": {},
                    "source": ["print('hello')\n"],
                    "outputs": [],
                    "execution_count": Value::Null
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5
        });
        std::fs::write(path, serde_json::to_string_pretty(&notebook).unwrap()).unwrap();
    }

    #[tokio::test]
    async fn replaces_existing_cell() {
        let dir = make_temp_dir("replace");
        let path = dir.join("test.ipynb");
        write_notebook(&path);

        let tool = NotebookEditTool::new();
        let result = tool
            .execute(
                json!({
                    "file_path": path,
                    "operation": "replace",
                    "index": 0,
                    "cell_type": "markdown",
                    "source": "# Title"
                }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..ToolContext::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("test.ipynb")).unwrap()).unwrap();
        assert_eq!(saved["cells"][0]["cell_type"], "markdown");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rejects_invalid_index() {
        let dir = make_temp_dir("invalid-index");
        let path = dir.join("test.ipynb");
        write_notebook(&path);

        let tool = NotebookEditTool::new();
        let err = tool
            .execute(
                json!({
                    "file_path": path,
                    "operation": "replace",
                    "index": 99
                }),
                ToolContext {
                    tool_use_id: "tool_2".to_string(),
                    ..ToolContext::default()
                },
            )
            .await
            .unwrap_err();

        assert!(err.to_string().contains("out of range"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
