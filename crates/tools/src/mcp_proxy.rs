//! MCP proxy tool — wraps a remote MCP tool as a local `Tool` implementation.
//!
//! Each MCP tool discovered from a connected server gets wrapped as a
//! `McpProxyTool` with the naming convention `mcp__<server>__<tool>`.

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use rust_claude_mcp::McpManager;
use std::sync::Arc;

use crate::tool::{Tool, ToolContext, ToolError};

/// A proxy tool that delegates execution to an MCP server via `McpManager`.
pub struct McpProxyTool {
    /// Fully qualified local name: `mcp__<server>__<tool>`.
    qualified_name: String,
    /// Human-readable description forwarded from the remote tool.
    description: String,
    /// JSON Schema for the tool's input parameters.
    input_schema: serde_json::Value,
    /// Shared reference to the MCP manager for dispatching calls.
    manager: Arc<McpManager>,
}

impl McpProxyTool {
    /// Create a new MCP proxy tool.
    pub fn new(
        qualified_name: String,
        description: String,
        input_schema: serde_json::Value,
        manager: Arc<McpManager>,
    ) -> Self {
        McpProxyTool {
            qualified_name,
            description,
            input_schema,
            manager,
        }
    }
}

#[async_trait]
impl Tool for McpProxyTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: self.qualified_name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
        }
    }

    /// MCP tools are considered non-read-only by default (safety first).
    fn is_read_only(&self) -> bool {
        false
    }

    /// MCP tools are not concurrency-safe by default.
    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        match self.manager.call_tool(&self.qualified_name, input).await {
            Ok(call_result) => {
                if call_result.is_error {
                    Ok(ToolResult::error(context.tool_use_id, call_result.content))
                } else {
                    Ok(ToolResult::success(
                        context.tool_use_id,
                        call_result.content,
                    ))
                }
            }
            Err(e) => Ok(ToolResult::error(
                context.tool_use_id,
                format!("MCP tool call failed: {}", e),
            )),
        }
    }
}

/// Register all discovered MCP tools from the manager into a `ToolRegistry`.
pub fn register_mcp_tools(registry: &mut crate::registry::ToolRegistry, manager: &Arc<McpManager>) {
    for (qualified_name, tool_info) in manager.discovered_tools() {
        let proxy = McpProxyTool::new(
            qualified_name,
            tool_info.description.clone(),
            tool_info.input_schema.clone(),
            manager.clone(),
        );
        registry.register(proxy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_tool_info() {
        let manager = Arc::new(McpManager::empty());
        let tool = McpProxyTool::new(
            "mcp__filesystem__read_file".into(),
            "Read a file from the filesystem".into(),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
            manager,
        );

        let info = tool.info();
        assert_eq!(info.name, "mcp__filesystem__read_file");
        assert_eq!(info.description, "Read a file from the filesystem");
        assert!(info.input_schema.is_object());
    }

    #[test]
    fn test_proxy_tool_safety_defaults() {
        let manager = Arc::new(McpManager::empty());
        let tool = McpProxyTool::new(
            "mcp__test__tool".into(),
            "Test".into(),
            serde_json::json!({}),
            manager,
        );

        assert!(!tool.is_read_only());
        assert!(!tool.is_concurrency_safe());
    }

    #[tokio::test]
    async fn test_proxy_tool_execute_tool_not_found() {
        let manager = Arc::new(McpManager::empty());
        let tool = McpProxyTool::new(
            "mcp__nonexistent__tool".into(),
            "Test".into(),
            serde_json::json!({}),
            manager,
        );

        let context = ToolContext {
            tool_use_id: "test_id".into(),
            app_state: None,
            agent_context: None,
            user_question_callback: None,
        };

        let result = tool.execute(serde_json::json!({}), context).await.unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("MCP tool call failed"));
    }

    #[test]
    fn test_proxy_tool_naming_convention() {
        // Verify the naming format: mcp__<server>__<tool>
        let name = format!("mcp__{}__{}", "filesystem", "read_file");
        assert_eq!(name, "mcp__filesystem__read_file");

        // Two servers with same tool name produce different qualified names
        let name1 = format!("mcp__{}__{}", "server_a", "search");
        let name2 = format!("mcp__{}__{}", "server_b", "search");
        assert_ne!(name1, name2);
    }

    #[test]
    fn test_register_mcp_tools_empty_manager() {
        let manager = Arc::new(McpManager::empty());
        let mut registry = crate::ToolRegistry::new();

        register_mcp_tools(&mut registry, &manager);

        // No tools should be registered
        assert!(registry.names().is_empty());
    }
}
