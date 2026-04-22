use async_trait::async_trait;
use rust_claude_core::state::AppState;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct AgentTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct AgentToolInput {
    prompt: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
}

impl AgentTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Agent".to_string(),
            description: "Spawn a sub-agent to handle a complex sub-task and return its final result"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" },
                    "allowed_tools": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["prompt"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: AgentToolInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let Some(agent_context) = context.agent_context.clone() else {
            return Ok(ToolResult::error(
                context.tool_use_id,
                "Agent context is not available".to_string(),
            ));
        };

        if agent_context.current_depth >= agent_context.max_depth {
            return Ok(ToolResult::error(
                context.tool_use_id,
                format!(
                    "Maximum agent nesting depth reached ({})",
                    agent_context.max_depth
                ),
            ));
        }

        let Some(parent_state) = context.app_state.clone() else {
            return Err(ToolError::Execution(
                "Agent requires app_state in tool context".to_string(),
            ));
        };

        let sub_state = {
            let parent = parent_state.lock().await;
            let mut sub = AppState::new(parent.cwd.clone());
            sub.permission_mode = parent.permission_mode;
            sub.always_allow_rules = parent.always_allow_rules.clone();
            sub.always_deny_rules = parent.always_deny_rules.clone();
            sub.session = parent.session.clone();
            Arc::new(Mutex::new(sub))
        };

        let allowed_tools = input.allowed_tools;
        let output = (agent_context.run_subagent)(
            input.prompt,
            allowed_tools,
            sub_state,
            agent_context.current_depth + 1,
            agent_context.max_depth,
        )
        .await;

        match output {
            Ok(output) => Ok(ToolResult::success(
                context.tool_use_id,
                format!(
                    "{}\n\nSub-agent usage:\n  input_tokens: {}\n  output_tokens: {}",
                    output.text, output.input_tokens, output.output_tokens
                ),
            )),
            Err(error) => Ok(ToolResult::error(
                context.tool_use_id,
                format!("Agent execution failed: {}", error),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentContext, ToolContext};

    fn app_state() -> Arc<Mutex<AppState>> {
        Arc::new(Mutex::new(AppState::new(std::path::PathBuf::from("/tmp"))))
    }

    #[tokio::test]
    async fn test_missing_agent_context_returns_tool_error_result() {
        let result = AgentTool::new()
            .execute(
                serde_json::json!({"prompt": "do work"}),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: Some(app_state()),
                    agent_context: None,
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("Agent context is not available"));
    }

    #[tokio::test]
    async fn test_depth_limit_returns_error_result() {
        let result = AgentTool::new()
            .execute(
                serde_json::json!({"prompt": "do work"}),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: Some(app_state()),
                    agent_context: Some(AgentContext {
                        current_depth: 3,
                        max_depth: 3,
                        ..Default::default()
                    }),
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("Maximum agent nesting depth reached"));
    }

    #[tokio::test]
    async fn test_successful_agent_execution() {
        let result = AgentTool::new()
            .execute(
                serde_json::json!({
                    "prompt": "summarize file",
                    "allowed_tools": ["FileRead"]
                }),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: Some(app_state()),
                    agent_context: Some(AgentContext {
                        run_subagent: Arc::new(|prompt, allowed_tools, _state, depth, max_depth| {
                            Box::pin(async move {
                                assert_eq!(prompt, "summarize file");
                                assert_eq!(allowed_tools, vec!["FileRead"]);
                                assert_eq!(depth, 1);
                                assert_eq!(max_depth, 3);
                                Ok(crate::tool::AgentRunOutput {
                                    text: "done".into(),
                                    input_tokens: 10,
                                    output_tokens: 5,
                                })
                            })
                        }),
                        ..Default::default()
                    }),
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("done"));
        assert!(result.content.contains("input_tokens: 10"));
    }
}
