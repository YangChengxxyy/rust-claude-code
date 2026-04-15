use async_trait::async_trait;
use rust_claude_core::state::AppState;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub tool_use_id: String,
    pub app_state: Option<Arc<Mutex<AppState>>>,
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
