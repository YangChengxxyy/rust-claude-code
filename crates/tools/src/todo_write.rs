use async_trait::async_trait;
use rust_claude_core::state::{TodoItem, TodoPriority, TodoStatus};
use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct TodoWriteTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct TodoWriteInput {
    todos: Vec<TodoEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TodoEntry {
    id: String,
    content: String,
    status: TodoStatus,
    priority: TodoPriority,
}

impl TodoWriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "TodoWrite".to_string(),
            description: "Update the todo list in app state".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "content": { "type": "string" },
                                "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] },
                                "priority": { "type": "string", "enum": ["high", "medium", "low"] }
                            },
                            "required": ["id", "content", "status", "priority"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: TodoWriteInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        let todos: Vec<TodoItem> = input
            .todos
            .into_iter()
            .map(|todo| TodoItem {
                id: todo.id,
                content: todo.content,
                status: todo.status,
                priority: todo.priority,
            })
            .collect();

        let Some(app_state) = context.app_state else {
            return Err(ToolError::Execution(
                "TodoWrite requires app_state in tool context".to_string(),
            ));
        };

        let mut app_state = app_state.lock().await;
        app_state.update_todos(todos.clone());

        Ok(ToolResult::success(
            context.tool_use_id,
            format!("Updated {} todos", todos.len()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::state::AppState;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_todo_write_updates_app_state() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));

        TodoWriteTool::new()
            .execute(
                serde_json::json!({
                    "todos": [
                        { "id": "1", "content": "task", "status": "pending", "priority": "high" }
                    ]
                }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: Some(app_state.clone()),
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        let state = app_state.lock().await;
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].content, "task");
    }

    #[tokio::test]
    async fn test_todo_write_clears_when_all_completed() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));

        TodoWriteTool::new()
            .execute(
                serde_json::json!({
                    "todos": [
                        { "id": "1", "content": "task", "status": "completed", "priority": "high" }
                    ]
                }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: Some(app_state.clone()),
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        let state = app_state.lock().await;
        assert!(state.tasks.is_empty());
    }

    #[tokio::test]
    async fn test_todo_write_requires_app_state() {
        let error = TodoWriteTool::new()
            .execute(
                serde_json::json!({
                    "todos": [
                        { "id": "1", "content": "task", "status": "pending", "priority": "high" }
                    ]
                }),
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
            matches!(error, ToolError::Execution(message) if message.contains("requires app_state"))
        );
    }
}
