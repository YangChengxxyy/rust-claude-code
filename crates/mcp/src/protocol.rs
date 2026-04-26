//! MCP protocol operations: initialize, tools/list, tools/call.

use rust_claude_core::mcp_config::{McpServerConfig, McpToolInfo};
use serde::{Deserialize, Serialize};

use crate::error::McpError;
use crate::jsonrpc::JsonRpcRequest;
use crate::transport::StdioTransport;

/// Client capabilities sent during initialization.
#[derive(Debug, Clone, Serialize)]
struct ClientCapabilities {
    // Empty for now — we only need tool calling.
}

/// Initialize request params.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    protocol_version: String,
    capabilities: ClientCapabilities,
    client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize)]
struct ClientInfo {
    name: String,
    version: String,
}

/// Server info from the initialize response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    #[serde(default)]
    pub protocol_version: String,
    #[serde(default)]
    pub server_info: Option<ServerInfo>,
    #[serde(default)]
    pub capabilities: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// A tool definition as returned by `tools/list`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// tools/list response.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsListResult {
    #[serde(default)]
    pub tools: Vec<McpToolDefinition>,
}

/// Content item from tools/call response.
#[derive(Debug, Clone, Deserialize)]
pub struct McpContentItem {
    #[serde(rename = "type", default)]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

/// tools/call response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCallResult {
    #[serde(default)]
    pub content: Vec<McpContentItem>,
    #[serde(default)]
    pub is_error: bool,
}

/// A connected MCP client wrapping a stdio transport.
pub struct McpClient {
    transport: StdioTransport,
    server_name: String,
    #[allow(dead_code)]
    initialize_result: Option<InitializeResult>,
}

impl McpClient {
    /// Connect to an MCP server using the given config.
    /// This starts the process and performs the `initialize` handshake.
    pub async fn connect(server_name: &str, config: &McpServerConfig) -> Result<Self, McpError> {
        let transport = StdioTransport::start(
            &config.command,
            &config.args,
            &config.env,
            config.cwd.as_deref(),
        )?;

        let mut client = McpClient {
            transport,
            server_name: server_name.to_string(),
            initialize_result: None,
        };

        client.initialize().await?;
        Ok(client)
    }

    /// Connect with a custom timeout (in milliseconds).
    pub async fn connect_with_timeout(
        server_name: &str,
        config: &McpServerConfig,
        timeout_ms: u64,
    ) -> Result<Self, McpError> {
        let transport = StdioTransport::start(
            &config.command,
            &config.args,
            &config.env,
            config.cwd.as_deref(),
        )?
        .with_timeout_ms(timeout_ms);

        let mut client = McpClient {
            transport,
            server_name: server_name.to_string(),
            initialize_result: None,
        };

        client.initialize().await?;
        Ok(client)
    }

    /// Send the `initialize` request and the `initialized` notification.
    async fn initialize(&mut self) -> Result<(), McpError> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities {},
            client_info: ClientInfo {
                name: "rust-claude-code".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let request = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::to_value(&params).map_err(|e| {
                McpError::Protocol(format!("failed to serialize initialize params: {e}"))
            })?),
        );

        let result_value = self.transport.send_request(&request).await?;
        let init_result: InitializeResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("invalid initialize response: {e}")))?;

        self.initialize_result = Some(init_result);

        // Send `initialized` notification (no response expected).
        // We write it directly, bypassing send_request since there's no response.
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        });
        let json = serde_json::to_vec(&notification).map_err(|e| {
            McpError::InvalidJson(format!("failed to serialize initialized notification: {e}"))
        })?;
        {
            let mut stdin = self.transport.stdin.lock().await;
            crate::jsonrpc::write_message(&mut *stdin, &json).await?;
        }

        Ok(())
    }

    /// Fetch the list of tools from the server.
    pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>, McpError> {
        let request = JsonRpcRequest::new("tools/list", Some(serde_json::json!({})));
        let result_value = self.transport.send_request(&request).await?;

        let tools_result: ToolsListResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("invalid tools/list response: {e}")))?;

        Ok(tools_result
            .tools
            .into_iter()
            .map(|t| McpToolInfo {
                name: t.name,
                description: t.description.unwrap_or_default(),
                input_schema: t.input_schema,
            })
            .collect())
    }

    /// Call a tool on the server.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult, McpError> {
        let request = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })),
        );

        let result_value = self.transport.send_request(&request).await?;

        let call_result: ToolsCallResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("invalid tools/call response: {e}")))?;

        // Concatenate all text content items
        let text = call_result
            .content
            .iter()
            .filter_map(|item| {
                if item.content_type == "text" {
                    item.text.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolCallResult {
            content: text,
            is_error: call_result.is_error,
        })
    }

    /// Get the server name.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Shutdown the server process.
    pub async fn shutdown(&mut self) {
        self.transport.shutdown().await;
    }
}

/// Result of a tools/call invocation.
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    /// The text content returned by the tool.
    pub content: String,
    /// Whether the tool call resulted in an error.
    pub is_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_deserialize() {
        let json = r#"{
            "name": "read_file",
            "description": "Read a file from the filesystem",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }
        }"#;

        let tool: McpToolDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "read_file");
        assert_eq!(
            tool.description.as_deref(),
            Some("Read a file from the filesystem")
        );
        assert!(tool.input_schema.is_object());
    }

    #[test]
    fn test_tools_list_result_deserialize() {
        let json = r#"{
            "tools": [
                {
                    "name": "read_file",
                    "description": "Read a file",
                    "inputSchema": {"type": "object"}
                },
                {
                    "name": "write_file",
                    "description": "Write a file",
                    "inputSchema": {"type": "object"}
                }
            ]
        }"#;

        let result: ToolsListResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.tools.len(), 2);
        assert_eq!(result.tools[0].name, "read_file");
        assert_eq!(result.tools[1].name, "write_file");
    }

    #[test]
    fn test_tools_list_result_empty() {
        let json = r#"{"tools": []}"#;
        let result: ToolsListResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.tools.len(), 0);
    }

    #[test]
    fn test_tools_call_result_success() {
        let json = r#"{
            "content": [
                {"type": "text", "text": "file contents here"}
            ],
            "isError": false
        }"#;

        let result: ToolsCallResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.content.len(), 1);
        assert_eq!(
            result.content[0].text.as_deref(),
            Some("file contents here")
        );
        assert!(!result.is_error);
    }

    #[test]
    fn test_tools_call_result_error() {
        let json = r#"{
            "content": [
                {"type": "text", "text": "File not found"}
            ],
            "isError": true
        }"#;

        let result: ToolsCallResult = serde_json::from_str(json).unwrap();
        assert!(result.is_error);
    }

    #[test]
    fn test_initialize_result_deserialize() {
        let json = r#"{
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": "test-server", "version": "1.0"},
            "capabilities": {"tools": {}}
        }"#;

        let result: InitializeResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.protocol_version, "2024-11-05");
        let info = result.server_info.unwrap();
        assert_eq!(info.name, "test-server");
    }
}
