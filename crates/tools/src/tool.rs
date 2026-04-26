use async_trait::async_trait;
use rust_claude_core::state::AppState;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ToolRegistry;

pub type AgentRunFuture = Pin<Box<dyn Future<Output = Result<AgentRunOutput, ToolError>> + Send>>;
pub type UserQuestionFuture = Pin<Box<dyn Future<Output = Option<AskUserQuestionResponse>> + Send>>;
pub type UserQuestionCallback =
    Arc<dyn Fn(AskUserQuestionRequest) -> UserQuestionFuture + Send + Sync>;

#[derive(Debug, Clone)]
pub struct AgentRunOutput {
    pub text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Context for spawning sub-agent QueryLoops from within a tool.
///
/// Carried inside `ToolContext` as an optional field. Only `AgentTool`
/// inspects this; all other tools ignore it.
#[derive(Clone)]
pub struct AgentContext {
    /// Factory that produces a fresh ToolRegistry for the sub-agent.
    pub tool_registry_factory: Arc<dyn Fn() -> ToolRegistry + Send + Sync>,
    /// CLI-provided callback that runs a sub-agent and returns its final output.
    pub run_subagent: Arc<
        dyn Fn(String, Vec<String>, Arc<Mutex<AppState>>, u32, u32) -> AgentRunFuture + Send + Sync,
    >,
    /// Current nesting depth (0 = top-level).
    pub current_depth: u32,
    /// Maximum allowed nesting depth.
    pub max_depth: u32,
}

impl std::fmt::Debug for AgentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentContext")
            .field("current_depth", &self.current_depth)
            .field("max_depth", &self.max_depth)
            .finish_non_exhaustive()
    }
}

impl Default for AgentContext {
    fn default() -> Self {
        AgentContext {
            tool_registry_factory: Arc::new(|| ToolRegistry::new()),
            run_subagent: Arc::new(|_, _, _, _, _| {
                Box::pin(async {
                    Err(ToolError::Execution(
                        "sub-agent runner not available".into(),
                    ))
                })
            }),
            current_depth: 0,
            max_depth: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AskUserQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AskUserQuestionRequest {
    pub question: String,
    pub options: Vec<AskUserQuestionOption>,
    pub allow_custom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AskUserQuestionResponse {
    pub selected_label: Option<String>,
    pub answer: String,
    pub custom: bool,
}

#[derive(Clone, Default)]
pub struct ToolContext {
    pub tool_use_id: String,
    pub app_state: Option<Arc<Mutex<AppState>>>,
    /// Context for spawning sub-agents. Only used by AgentTool.
    pub agent_context: Option<AgentContext>,
    /// Optional UI/backend callback for structured user questions.
    pub user_question_callback: Option<UserQuestionCallback>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("tool_use_id", &self.tool_use_id)
            .field(
                "app_state",
                &self.app_state.as_ref().map(|_| "Some(AppState)"),
            )
            .field("agent_context", &self.agent_context)
            .field(
                "user_question_callback",
                &self
                    .user_question_callback
                    .as_ref()
                    .map(|_| "Some(callback)"),
            )
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("execution failed: {0}")]
    Execution(String),
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn info(&self) -> ToolInfo;

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
    ) -> Result<ToolResult, ToolError>;
}
