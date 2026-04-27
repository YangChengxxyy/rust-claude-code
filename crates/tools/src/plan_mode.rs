use async_trait::async_trait;
use rust_claude_core::permission::PermissionMode;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct EnterPlanModeTool;

#[derive(Debug, Clone, Default)]
pub struct ExitPlanModeTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct ExitPlanModeInput {
    plan: String,
}

impl EnterPlanModeTool {
    pub fn new() -> Self {
        Self
    }
}

impl ExitPlanModeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "EnterPlanMode".to_string(),
            description: "Switch the session to plan mode and save the previous permission mode"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let Some(app_state) = context.app_state else {
            return Err(ToolError::Execution(
                "EnterPlanMode requires app_state in tool context".to_string(),
            ));
        };

        let mut state = app_state.lock().await;
        if state.permission_mode == PermissionMode::Plan {
            return Ok(ToolResult::success(
                context.tool_use_id,
                "Already in plan mode; no transition needed.",
            ));
        }

        let previous = state.permission_mode;
        state.enter_plan_mode();
        Ok(ToolResult::success(
            context.tool_use_id,
            format!(
                "Entered plan mode. Previous permission mode: {:?}.",
                previous
            ),
        ))
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "ExitPlanMode".to_string(),
            description: "Exit plan mode with a summary and restore the saved permission mode"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "plan": { "type": "string", "description": "Non-empty plan summary" }
                },
                "required": ["plan"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: ExitPlanModeInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        let plan = input.plan.trim();
        if plan.is_empty() {
            return Err(ToolError::InvalidInput(
                "plan summary cannot be empty".to_string(),
            ));
        }

        let Some(app_state) = context.app_state else {
            return Err(ToolError::Execution(
                "ExitPlanMode requires app_state in tool context".to_string(),
            ));
        };

        let mut state = app_state.lock().await;
        match state.exit_plan_mode() {
            Some(restored) => Ok(ToolResult::success(
                context.tool_use_id,
                format!("Exited plan mode. Restored permission mode: {:?}. Plan summary:\n{}", restored, plan),
            )),
            None => Ok(ToolResult::success(
                context.tool_use_id,
                format!("No active plan-mode transition was found. Current permission mode unchanged. Plan summary:\n{}", plan),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::state::AppState;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn context(app_state: Arc<Mutex<AppState>>) -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            app_state: Some(app_state),
            agent_context: None,
            user_question_callback: None,
        }
    }

    #[tokio::test]
    async fn test_enter_plan_mode_saves_previous_mode() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        app_state.lock().await.permission_mode = PermissionMode::AcceptEdits;

        let result = EnterPlanModeTool::new()
            .execute(serde_json::json!({}), context(app_state.clone()))
            .await
            .unwrap();

        assert!(result.content.contains("Entered plan mode"));
        let state = app_state.lock().await;
        assert_eq!(state.permission_mode, PermissionMode::Plan);
        assert_eq!(
            state.previous_permission_mode,
            Some(PermissionMode::AcceptEdits)
        );
    }

    #[tokio::test]
    async fn test_enter_plan_mode_when_already_plan_is_idempotent() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        app_state.lock().await.permission_mode = PermissionMode::Plan;

        let result = EnterPlanModeTool::new()
            .execute(serde_json::json!({}), context(app_state.clone()))
            .await
            .unwrap();

        assert!(result.content.contains("Already in plan mode"));
        let state = app_state.lock().await;
        assert_eq!(state.permission_mode, PermissionMode::Plan);
        assert_eq!(state.previous_permission_mode, None);
    }

    #[tokio::test]
    async fn test_exit_plan_mode_restores_saved_mode_with_summary() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        {
            let mut state = app_state.lock().await;
            state.permission_mode = PermissionMode::Plan;
            state.previous_permission_mode = Some(PermissionMode::AcceptEdits);
        }

        let result = ExitPlanModeTool::new()
            .execute(
                serde_json::json!({ "plan": "Implement the change in small steps." }),
                context(app_state.clone()),
            )
            .await
            .unwrap();

        assert!(result.content.contains("Implement the change"));
        let state = app_state.lock().await;
        assert_eq!(state.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(state.previous_permission_mode, None);
    }

    #[tokio::test]
    async fn test_exit_plan_mode_without_saved_mode_reports_no_active_transition() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));
        app_state.lock().await.permission_mode = PermissionMode::Plan;

        let result = ExitPlanModeTool::new()
            .execute(
                serde_json::json!({ "plan": "No changes needed." }),
                context(app_state.clone()),
            )
            .await
            .unwrap();

        assert!(result.content.contains("No active plan-mode transition"));
        assert_eq!(app_state.lock().await.permission_mode, PermissionMode::Plan);
    }

    #[tokio::test]
    async fn test_exit_plan_mode_rejects_empty_summary() {
        let app_state = Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))));

        let error = ExitPlanModeTool::new()
            .execute(serde_json::json!({ "plan": "  " }), context(app_state))
            .await
            .unwrap_err();

        assert!(
            matches!(error, ToolError::InvalidInput(message) if message.contains("cannot be empty"))
        );
    }
}
