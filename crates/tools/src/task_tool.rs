use async_trait::async_trait;
use rust_claude_core::state::{Task, TaskPriority, TaskStatus};
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use uuid::Uuid;

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct TaskTool;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
enum TaskInput {
    Create {
        content: String,
        #[serde(default = "default_priority")]
        priority: TaskPriority,
    },
    List,
    Update {
        id: String,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        status: Option<TaskStatus>,
        #[serde(default)]
        priority: Option<TaskPriority>,
    },
    Get {
        id: String,
    },
}

fn default_priority() -> TaskPriority {
    TaskPriority::Medium
}

impl TaskTool {
    pub fn new() -> Self {
        Self
    }

    fn format_task(task: &Task) -> String {
        format!(
            "Task {}\n  content: {}\n  status: {:?}\n  priority: {:?}",
            task.id, task.content, task.status, task.priority
        )
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Task".to_string(),
            description: "Manage tasks in app state: create, list, update, get".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "enum": ["create", "list", "update", "get"] },
                    "content": { "type": "string" },
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"] },
                    "priority": { "type": "string", "enum": ["high", "medium", "low"] }
                },
                "required": ["command"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: TaskInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let Some(app_state) = context.app_state else {
            return Err(ToolError::Execution(
                "Task requires app_state in tool context".to_string(),
            ));
        };

        let mut app_state = app_state.lock().await;

        match input {
            TaskInput::Create { content, priority } => {
                let task = Task {
                    id: format!("task_{}", Uuid::new_v4().simple()),
                    content,
                    status: TaskStatus::Pending,
                    priority,
                };
                app_state.tasks.push(task.clone());
                Ok(ToolResult::success(
                    context.tool_use_id,
                    format!("Created task\n{}", Self::format_task(&task)),
                ))
            }
            TaskInput::List => {
                if app_state.tasks.is_empty() {
                    return Ok(ToolResult::success(context.tool_use_id, "No tasks"));
                }
                let body = app_state
                    .tasks
                    .iter()
                    .map(Self::format_task)
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Ok(ToolResult::success(context.tool_use_id, body))
            }
            TaskInput::Update {
                id,
                content,
                status,
                priority,
            } => {
                let task = app_state
                    .tasks
                    .iter_mut()
                    .find(|task| task.id == id)
                    .ok_or_else(|| ToolError::Execution(format!("task not found: {}", id)))?;

                if let Some(content) = content {
                    task.content = content;
                }
                if let Some(status) = status {
                    task.status = status;
                }
                if let Some(priority) = priority {
                    task.priority = priority;
                }

                Ok(ToolResult::success(
                    context.tool_use_id,
                    format!("Updated task\n{}", Self::format_task(task)),
                ))
            }
            TaskInput::Get { id } => {
                let task = app_state
                    .tasks
                    .iter()
                    .find(|task| task.id == id)
                    .ok_or_else(|| ToolError::Execution(format!("task not found: {}", id)))?;
                Ok(ToolResult::success(
                    context.tool_use_id,
                    Self::format_task(task),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::state::AppState;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn context(app_state: &Arc<Mutex<AppState>>) -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            app_state: Some(app_state.clone()),
            agent_context: None,
            user_question_callback: None,
        }
    }

    #[tokio::test]
    async fn test_create_task() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        let result = TaskTool::new()
            .execute(
                serde_json::json!({
                    "command": "create",
                    "content": "implement login",
                    "priority": "high"
                }),
                context(&app_state),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let state = app_state.lock().await;
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].content, "implement login");
        assert_eq!(state.tasks[0].status, TaskStatus::Pending);
        assert_eq!(state.tasks[0].priority, TaskPriority::High);
    }

    #[tokio::test]
    async fn test_list_tasks_empty() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        let result = TaskTool::new()
            .execute(serde_json::json!({"command": "list"}), context(&app_state))
            .await
            .unwrap();
        assert_eq!(result.content, "No tasks");
    }

    #[tokio::test]
    async fn test_update_task() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut state = app_state.lock().await;
            state.tasks.push(Task {
                id: "task_1".into(),
                content: "original".into(),
                status: TaskStatus::Pending,
                priority: TaskPriority::Medium,
            });
        }

        let result = TaskTool::new()
            .execute(
                serde_json::json!({
                    "command": "update",
                    "id": "task_1",
                    "status": "completed",
                    "priority": "low"
                }),
                context(&app_state),
            )
            .await
            .unwrap();

        assert!(result.content.contains("Updated task"));
        let state = app_state.lock().await;
        assert_eq!(state.tasks[0].status, TaskStatus::Completed);
        assert_eq!(state.tasks[0].priority, TaskPriority::Low);
    }

    #[tokio::test]
    async fn test_get_task() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut state = app_state.lock().await;
            state.tasks.push(Task {
                id: "task_1".into(),
                content: "inspect logs".into(),
                status: TaskStatus::InProgress,
                priority: TaskPriority::Medium,
            });
        }

        let result = TaskTool::new()
            .execute(
                serde_json::json!({"command": "get", "id": "task_1"}),
                context(&app_state),
            )
            .await
            .unwrap();
        assert!(result.content.contains("inspect logs"));
    }

    #[tokio::test]
    async fn test_invalid_command() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        let error = TaskTool::new()
            .execute(serde_json::json!({"command": "boom"}), context(&app_state))
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_missing_app_state() {
        let error = TaskTool::new()
            .execute(
                serde_json::json!({"command": "list"}),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::Execution(_)));
    }
}
