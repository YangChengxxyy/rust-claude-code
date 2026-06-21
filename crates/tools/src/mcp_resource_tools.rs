//! `ListMcpResources` / `ReadMcpResource` — browse and read MCP server
//! resources (iteration 53).
//!
//! These tools wrap `McpManager::list_server_resources` /
//! `read_server_resource`. The manager routing and JSON-RPC plumbing live in
//! the `mcp` crate; the logic here is the tool surface: input parsing,
//! stable-text rendering, and friendly error messages. Rendering and error
//! mapping are factored into pure helpers so they can be unit-tested without
//! a live server.

use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use rust_claude_mcp::protocol::{
    McpResource, ReadResourceResult, ResourceContent, METHOD_NOT_FOUND_CODE,
};
use rust_claude_mcp::{McpError, McpManager};
use serde::Deserialize;
use std::sync::Arc;

use crate::tool::{Tool, ToolContext, ToolError};

/// List the resources exposed by a connected MCP server.
pub struct ListMcpResourcesTool {
    manager: Arc<McpManager>,
}

/// Read a single resource by URI from a connected MCP server.
pub struct ReadMcpResourceTool {
    manager: Arc<McpManager>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListResourcesInput {
    server: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ReadResourceInput {
    server: String,
    uri: String,
}

impl ListMcpResourcesTool {
    pub fn new(manager: Arc<McpManager>) -> Self {
        Self { manager }
    }
}

impl ReadMcpResourceTool {
    pub fn new(manager: Arc<McpManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "ListMcpResources".to_string(),
            description: "List resources exposed by a connected MCP server via `resources/list`. \
                Returns each resource's URI, name, and description. A server that exposes no \
                resources — or does not implement them at all — produces a clear message rather \
                than a failure."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "server": {
                        "type": "string",
                        "description": "Name of the connected MCP server to list resources from."
                    }
                },
                "required": ["server"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: ListResourcesInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        if input.server.trim().is_empty() {
            return Err(ToolError::InvalidInput("'server' cannot be empty".into()));
        }
        match self.manager.list_server_resources(&input.server).await {
            Ok(resources) => Ok(ToolResult::success(
                context.tool_use_id,
                render_resource_list(&input.server, &resources),
            )),
            Err(err) => Ok(ToolResult::error(
                context.tool_use_id,
                resource_error_message(&input.server, &err),
            )),
        }
    }
}

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "ReadMcpResource".to_string(),
            description: "Read a resource by URI from a connected MCP server via \
                `resources/read`. Textual resources are returned as text; binary (blob) resources \
                are reported as a placeholder. Servers that do not implement resources produce a \
                clear error."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "server": {
                        "type": "string",
                        "description": "Name of the connected MCP server that owns the resource."
                    },
                    "uri": {
                        "type": "string",
                        "description": "URI of the resource to read."
                    }
                },
                "required": ["server", "uri"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: ReadResourceInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        if input.server.trim().is_empty() {
            return Err(ToolError::InvalidInput("'server' cannot be empty".into()));
        }
        if input.uri.trim().is_empty() {
            return Err(ToolError::InvalidInput("'uri' cannot be empty".into()));
        }
        match self.manager.read_server_resource(&input.server, &input.uri).await {
            Ok(result) => Ok(ToolResult::success(
                context.tool_use_id,
                render_resource_contents(&result),
            )),
            Err(err) => Ok(ToolResult::error(
                context.tool_use_id,
                resource_error_message(&input.server, &err),
            )),
        }
    }
}

// ── Pure rendering / error helpers ──

/// Render a `resources/list` result as stable text. An empty list yields an
/// explicit, non-error message.
pub fn render_resource_list(server: &str, resources: &[McpResource]) -> String {
    if resources.is_empty() {
        return format!("MCP server '{}' exposes no resources.", server);
    }
    let mut lines = Vec::with_capacity(resources.len() + 1);
    lines.push(format!(
        "Resources from MCP server '{}' ({}):",
        server,
        resources.len()
    ));
    for r in resources {
        let desc = match r.description.as_deref() {
            Some(d) if !d.is_empty() => format!(" — {d}"),
            _ => String::new(),
        };
        lines.push(format!("- {} ({}){desc}", r.uri, r.name));
    }
    lines.join("\n")
}

/// Render a `resources/read` result as stable text. Text content is passed
/// through; base64 blobs become a placeholder; an empty result is explicit.
pub fn render_resource_contents(result: &ReadResourceResult) -> String {
    if result.contents.is_empty() {
        return "(resource returned no content)".to_string();
    }
    let parts: Vec<String> = result
        .contents
        .iter()
        .map(|c| render_single_content(c))
        .collect();
    parts.join("\n")
}

fn render_single_content(content: &ResourceContent) -> String {
    if let Some(text) = &content.text {
        return text.clone();
    }
    if let Some(blob) = &content.blob {
        return format!("[binary blob: {} base64 chars]", blob.len());
    }
    format!("[empty content for {}]", content.uri)
}

/// Map an MCP error from a resource operation to a clear, user-facing message.
pub fn resource_error_message(server: &str, err: &McpError) -> String {
    match err {
        McpError::ServerNotConnected(_) => {
            format!("MCP server '{}' is not connected.", server)
        }
        McpError::JsonRpcError { code, .. } if *code == METHOD_NOT_FOUND_CODE => {
            format!(
                "MCP server '{}' does not support resources (method not found).",
                server
            )
        }
        other => format!("MCP resource operation on '{}' failed: {}", server, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ToolContext {
        ToolContext {
            tool_use_id: "t".into(),
            app_state: None,
            agent_context: None,
            user_question_callback: None,
            ..Default::default()
        }
    }

    fn res(uri: &str, name: &str, desc: Option<&str>) -> McpResource {
        McpResource {
            uri: uri.into(),
            name: name.into(),
            description: desc.map(Into::into),
            mime_type: None,
        }
    }

    // ── render_resource_list ──

    #[test]
    fn render_list_empty_is_clear_and_non_error() {
        let out = render_resource_list("docs", &[]);
        assert!(out.contains("no resources"));
        assert!(out.contains("docs"));
    }

    #[test]
    fn render_list_populated_shows_entries() {
        let resources = vec![
            res("file:///a", "a", Some("A file")),
            res("file:///b", "b", None),
        ];
        let out = render_resource_list("docs", &resources);
        assert!(out.contains("Resources from MCP server 'docs' (2):"));
        assert!(out.contains("file:///a (a) — A file"));
        assert!(out.contains("file:///b (b)"));
        // No dangling separator when description is absent.
        assert!(!out.contains("(b) —\n"));
    }

    // ── render_resource_contents ──

    #[test]
    fn render_contents_text_passthrough() {
        let result = ReadResourceResult {
            contents: vec![ResourceContent {
                uri: "file:///a".into(),
                mime_type: Some("text/plain".into()),
                text: Some("hello world".into()),
                blob: None,
            }],
        };
        assert_eq!(render_resource_contents(&result), "hello world");
    }

    #[test]
    fn render_contents_blob_placeholder() {
        let result = ReadResourceResult {
            contents: vec![ResourceContent {
                uri: "img:///x".into(),
                mime_type: None,
                text: None,
                blob: Some("AAEC".into()),
            }],
        };
        let out = render_resource_contents(&result);
        assert!(out.contains("binary blob"));
        assert!(out.contains("4 base64 chars"));
    }

    #[test]
    fn render_contents_empty_is_clear() {
        let result = ReadResourceResult { contents: vec![] };
        assert_eq!(render_resource_contents(&result), "(resource returned no content)");
    }

    // ── resource_error_message ──

    #[test]
    fn error_message_not_connected() {
        let msg = resource_error_message("ghost", &McpError::ServerNotConnected("ghost".into()));
        assert!(msg.contains("not connected"));
        assert!(msg.contains("ghost"));
    }

    #[test]
    fn error_message_unsupported_resources() {
        let err = McpError::JsonRpcError {
            code: METHOD_NOT_FOUND_CODE,
            message: "Method not found".into(),
        };
        let msg = resource_error_message("svc", &err);
        assert!(msg.contains("does not support resources"));
        assert!(msg.contains("svc"));
    }

    #[test]
    fn error_message_other_is_generic() {
        let err = McpError::Timeout;
        let msg = resource_error_message("svc", &err);
        assert!(msg.contains("failed"));
    }

    // ── tool metadata ──

    #[test]
    fn list_tool_metadata() {
        let tool = ListMcpResourcesTool::new(Arc::new(McpManager::empty()));
        assert_eq!(tool.info().name, "ListMcpResources");
        assert!(tool.is_read_only());
        assert!(tool.is_concurrency_safe());
    }

    #[test]
    fn read_tool_metadata() {
        let tool = ReadMcpResourceTool::new(Arc::new(McpManager::empty()));
        assert_eq!(tool.info().name, "ReadMcpResource");
        assert!(tool.is_read_only());
        assert!(tool.is_concurrency_safe());
    }

    // ── execute with empty manager → not connected ──

    #[tokio::test]
    async fn list_execute_unknown_server_reports_not_connected() {
        let tool = ListMcpResourcesTool::new(Arc::new(McpManager::empty()));
        let result = tool
            .execute(serde_json::json!({ "server": "ghost" }), ctx())
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("not connected"));
    }

    #[tokio::test]
    async fn read_execute_unknown_server_reports_not_connected() {
        let tool = ReadMcpResourceTool::new(Arc::new(McpManager::empty()));
        let result = tool
            .execute(
                serde_json::json!({ "server": "ghost", "uri": "file:///x" }),
                ctx(),
            )
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("not connected"));
    }

    #[tokio::test]
    async fn list_execute_rejects_missing_server() {
        let tool = ListMcpResourcesTool::new(Arc::new(McpManager::empty()));
        let err = tool
            .execute(serde_json::json!({}), ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn read_execute_rejects_empty_uri() {
        let tool = ReadMcpResourceTool::new(Arc::new(McpManager::empty()));
        let err = tool
            .execute(
                serde_json::json!({ "server": "svc", "uri": "   " }),
                ctx(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    // ── registration ──

    #[test]
    fn register_mcp_tools_also_registers_resource_tools() {
        let manager = Arc::new(McpManager::empty());
        let registry = crate::ToolRegistry::new();
        crate::mcp_proxy::register_mcp_tools(&registry, &manager);

        let names = registry.names();
        assert!(names.iter().any(|n| n == "ListMcpResources"));
        assert!(names.iter().any(|n| n == "ReadMcpResource"));
    }
}
