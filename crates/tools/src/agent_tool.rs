use async_trait::async_trait;
use rust_claude_core::state::AppState;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tool::{AgentRunOptions, Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct AgentTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct AgentToolInput {
    prompt: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    agent: Option<String>,
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
            description:
                "Spawn a sub-agent to handle a complex sub-task and return its final result"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" },
                    "allowed_tools": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "agent": { "type": "string" }
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

        let mut allowed_tools = input.allowed_tools;
        let mut options = AgentRunOptions::default();
        if let Some(agent_name) = input
            .agent
            .as_deref()
            .filter(|name| !name.trim().is_empty())
        {
            let Some(agent) = agent_context.custom_agents.get(agent_name) else {
                return Ok(ToolResult::error(
                    context.tool_use_id,
                    format!("Custom agent not found: {agent_name}"),
                ));
            };

            options.system_prompt = Some(agent.system_prompt.clone());
            options.model = agent.model.clone();
            allowed_tools = intersect_allowed_tools(&agent.tools, &allowed_tools);
        }

        let output = (agent_context.run_subagent)(
            input.prompt,
            allowed_tools,
            options,
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

fn intersect_allowed_tools(agent_tools: &[String], explicit_tools: &[String]) -> Vec<String> {
    if agent_tools.is_empty() {
        return explicit_tools.to_vec();
    }
    if explicit_tools.is_empty() {
        return agent_tools.to_vec();
    }

    agent_tools
        .iter()
        .filter(|tool| explicit_tools.contains(tool))
        .cloned()
        .collect()
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
                    user_question_callback: None,
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
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result
            .content
            .contains("Maximum agent nesting depth reached"));
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
                        run_subagent: Arc::new(
                            |prompt, allowed_tools, options, _state, depth, max_depth| {
                                Box::pin(async move {
                                    assert_eq!(prompt, "summarize file");
                                    assert_eq!(allowed_tools, vec!["FileRead"]);
                                    assert_eq!(options, AgentRunOptions::default());
                                    assert_eq!(depth, 1);
                                    assert_eq!(max_depth, 3);
                                    Ok(crate::tool::AgentRunOutput {
                                        text: "done".into(),
                                        input_tokens: 10,
                                        output_tokens: 5,
                                    })
                                })
                            },
                        ),
                        ..Default::default()
                    }),
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("done"));
        assert!(result.content.contains("input_tokens: 10"));
    }

    #[tokio::test]
    async fn test_missing_custom_agent_returns_error() {
        let result = AgentTool::new()
            .execute(
                serde_json::json!({"prompt": "review", "agent": "missing-agent"}),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: Some(app_state()),
                    agent_context: Some(AgentContext::default()),
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("Custom agent not found"));
    }

    #[tokio::test]
    async fn test_custom_agent_applies_options_and_tool_intersection() {
        let registry = rust_claude_core::custom_agents::CustomAgentRegistry::from_agents(vec![
            rust_claude_core::custom_agents::CustomAgentDefinition {
                name: "reviewer".into(),
                description: "Reviews code".into(),
                system_prompt: "Review carefully".into(),
                tools: vec!["FileRead".into(), "Bash".into()],
                model: Some("model-x".into()),
                path: std::path::PathBuf::from("reviewer.md"),
            },
        ]);

        let result = AgentTool::new()
            .execute(
                serde_json::json!({
                    "prompt": "review",
                    "agent": "reviewer",
                    "allowed_tools": ["FileRead"]
                }),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: Some(app_state()),
                    agent_context: Some(AgentContext {
                        custom_agents: Arc::new(registry),
                        run_subagent: Arc::new(
                            |prompt, allowed_tools, options, _state, _depth, _max_depth| {
                                Box::pin(async move {
                                    assert_eq!(prompt, "review");
                                    assert_eq!(allowed_tools, vec!["FileRead"]);
                                    assert_eq!(
                                        options.system_prompt.as_deref(),
                                        Some("Review carefully")
                                    );
                                    assert_eq!(options.model.as_deref(), Some("model-x"));
                                    Ok(crate::tool::AgentRunOutput {
                                        text: "done".into(),
                                        input_tokens: 1,
                                        output_tokens: 2,
                                    })
                                })
                            },
                        ),
                        ..Default::default()
                    }),
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("done"));
    }

    #[tokio::test]
    async fn test_nested_agent_uses_incremented_depth() {
        let result = AgentTool::new()
            .execute(
                serde_json::json!({"prompt": "delegate"}),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    app_state: Some(app_state()),
                    agent_context: Some(AgentContext {
                        current_depth: 1,
                        max_depth: 3,
                        run_subagent: Arc::new(
                            |_prompt, _allowed_tools, _options, _state, depth, max_depth| {
                                Box::pin(async move {
                                    assert_eq!(depth, 2);
                                    assert_eq!(max_depth, 3);
                                    Ok(crate::tool::AgentRunOutput {
                                        text: "nested".into(),
                                        input_tokens: 1,
                                        output_tokens: 1,
                                    })
                                })
                            },
                        ),
                        ..Default::default()
                    }),
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("nested"));
    }

    #[test]
    fn test_custom_agent_tool_allowlist_cannot_be_broadened() {
        let agent_tools = vec!["FileRead".to_string()];
        let explicit = vec!["FileRead".to_string(), "Bash".to_string()];
        assert_eq!(
            intersect_allowed_tools(&agent_tools, &explicit),
            vec!["FileRead"]
        );
    }
}
