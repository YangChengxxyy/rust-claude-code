//! MCP protocol operations: initialize, tools/list, tools/call,
//! resources/list, resources/read.

use rust_claude_core::mcp_config::{McpServerConfig, McpToolInfo};
use serde::{Deserialize, Serialize};

use crate::error::McpError;
use crate::jsonrpc::JsonRpcRequest;
use crate::transport::{McpTransport, StdioTransport};

/// JSON-RPC method-not-found error code. Servers return this when a request
/// targets an optional capability they do not implement (e.g. a server with no
/// resource support replying to `resources/list`).
pub const METHOD_NOT_FOUND_CODE: i64 = -32601;

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

/// A resource exposed by an MCP server, as returned by `resources/list`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct McpResource {
    pub uri: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// `resources/list` response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesListResult {
    #[serde(default)]
    pub resources: Vec<McpResource>,
    /// Opaque pagination cursor; servers omit it when there are no more pages.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// One content entry in a `resources/read` response. A resource is either
/// textual (`text`) or binary (`blob`, base64-encoded).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
}

/// `resources/read` response.
#[derive(Debug, Clone, Deserialize)]
pub struct ReadResourceResult {
    #[serde(default)]
    pub contents: Vec<ResourceContent>,
}

/// A connected MCP client wrapping a stdio transport.
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    server_name: String,
    #[allow(dead_code)]
    initialize_result: Option<InitializeResult>,
}

impl McpClient {
    pub fn from_transport(server_name: &str, transport: Box<dyn McpTransport>) -> Self {
        McpClient {
            transport,
            server_name: server_name.to_string(),
            initialize_result: None,
        }
    }

    /// Connect to an MCP server using the given config.
    /// This starts the process and performs the `initialize` handshake.
    pub async fn connect(server_name: &str, config: &McpServerConfig) -> Result<Self, McpError> {
        let transport = StdioTransport::start(
            &config.command,
            &config.args,
            &config.env,
            config.cwd.as_deref(),
        )?;

        let mut client = McpClient::from_transport(server_name, Box::new(transport));

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

        let mut client = McpClient::from_transport(server_name, Box::new(transport));

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
            self.transport.send_notification(&json).await?;
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

    /// Fetch the list of resources from the server (`resources/list`).
    ///
    /// Servers that do not implement resources return a JSON-RPC
    /// method-not-found error (-32601), surfaced here as
    /// [`McpError::JsonRpcError`].
    pub async fn list_resources(&self) -> Result<Vec<McpResource>, McpError> {
        let request = JsonRpcRequest::new("resources/list", Some(serde_json::json!({})));
        let result_value = self.transport.send_request(&request).await?;

        let list_result: ResourcesListResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("invalid resources/list response: {e}")))?;

        Ok(list_result.resources)
    }

    /// Read a resource by URI (`resources/read`). Returns the raw content
    /// entries; callers decide how to render text vs. blob.
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, McpError> {
        let request = JsonRpcRequest::new(
            "resources/read",
            Some(serde_json::json!({ "uri": uri })),
        );
        let result_value = self.transport.send_request(&request).await?;

        let read_result: ReadResourceResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("invalid resources/read response: {e}")))?;

        Ok(read_result)
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
    use crate::transport::McpTransport;
    use std::sync::Arc;
    use tokio::sync::Mutex;

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

    struct FakeTransport {
        requests: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl McpTransport for FakeTransport {
        async fn send_request(
            &self,
            request: &JsonRpcRequest,
        ) -> Result<serde_json::Value, McpError> {
            self.requests.lock().await.push(request.method.clone());
            Ok(match request.method.as_str() {
                "tools/list" => serde_json::json!({
                    "tools": [{"name": "lookup", "description": "Lookup", "inputSchema": {"type": "object"}}]
                }),
                "tools/call" => serde_json::json!({
                    "content": [{"type": "text", "text": "ok"}],
                    "isError": false
                }),
                "resources/list" => serde_json::json!({
                    "resources": [
                        {"uri": "file:///a.txt", "name": "a", "description": "A file", "mimeType": "text/plain"},
                        {"uri": "file:///b.txt", "name": "b"}
                    ]
                }),
                "resources/read" => serde_json::json!({
                    "contents": [{"uri": "file:///a.txt", "mimeType": "text/plain", "text": "hello"}]
                }),
                method => panic!("unexpected method: {method}"),
            })
        }

        async fn send_notification(&self, _json: &[u8]) -> Result<(), McpError> {
            Ok(())
        }

        async fn shutdown(&mut self) {}
    }

    #[tokio::test]
    async fn test_client_uses_transport_trait_for_tools() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = McpClient::from_transport(
            "fake",
            Box::new(FakeTransport {
                requests: requests.clone(),
            }),
        );

        let tools = client.list_tools().await.unwrap();
        let result = client
            .call_tool("lookup", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(tools[0].name, "lookup");
        assert_eq!(result.content, "ok");
        assert_eq!(
            *requests.lock().await,
            vec!["tools/list".to_string(), "tools/call".to_string()]
        );
    }

    // ── resources/list, resources/read ──

    #[test]
    fn test_resources_list_result_deserialize() {
        let json = r#"{
            "resources": [
                {"uri": "file:///a.txt", "name": "a", "description": "A", "mimeType": "text/plain"},
                {"uri": "file:///b.txt", "name": "b"}
            ],
            "nextCursor": "page2"
        }"#;
        let result: ResourcesListResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.resources.len(), 2);
        assert_eq!(result.resources[0].uri, "file:///a.txt");
        assert_eq!(result.resources[0].mime_type.as_deref(), Some("text/plain"));
        assert_eq!(result.resources[1].description, None);
        assert_eq!(result.next_cursor.as_deref(), Some("page2"));
    }

    #[test]
    fn test_resources_list_result_empty() {
        let json = r#"{"resources": []}"#;
        let result: ResourcesListResult = serde_json::from_str(json).unwrap();
        assert!(result.resources.is_empty());
        assert!(result.next_cursor.is_none());
    }

    #[test]
    fn test_read_resource_result_deserialize_text_and_blob() {
        let json = r#"{
            "contents": [
                {"uri": "file:///a.txt", "mimeType": "text/plain", "text": "hello"},
                {"uri": "img:///b.bin", "mimeType": "application/octet-stream", "blob": "AAEC"}
            ]
        }"#;
        let result: ReadResourceResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.contents.len(), 2);
        assert_eq!(result.contents[0].text.as_deref(), Some("hello"));
        assert!(result.contents[0].blob.is_none());
        assert_eq!(result.contents[1].blob.as_deref(), Some("AAEC"));
        assert!(result.contents[1].text.is_none());
    }

    #[test]
    fn test_read_resource_result_empty() {
        let json = r#"{"contents": []}"#;
        let result: ReadResourceResult = serde_json::from_str(json).unwrap();
        assert!(result.contents.is_empty());
    }

    /// Transport that reports `resources/list` and `resources/read` as
    /// unsupported via a JSON-RPC method-not-found error.
    struct UnsupportedResourcesTransport {
        requests: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl McpTransport for UnsupportedResourcesTransport {
        async fn send_request(
            &self,
            request: &JsonRpcRequest,
        ) -> Result<serde_json::Value, McpError> {
            self.requests.lock().await.push(request.method.clone());
            match request.method.as_str() {
                "resources/list" | "resources/read" => Err(McpError::JsonRpcError {
                    code: METHOD_NOT_FOUND_CODE,
                    message: "Method not found".to_string(),
                }),
                method => panic!("unexpected method: {method}"),
            }
        }

        async fn send_notification(&self, _json: &[u8]) -> Result<(), McpError> {
            Ok(())
        }

        async fn shutdown(&mut self) {}
    }

    #[tokio::test]
    async fn test_client_lists_and_reads_resources_via_transport() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = McpClient::from_transport(
            "fake",
            Box::new(FakeTransport {
                requests: requests.clone(),
            }),
        );

        let resources = client.list_resources().await.unwrap();
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].uri, "file:///a.txt");
        assert_eq!(resources[0].name, "a");
        assert_eq!(resources[0].description.as_deref(), Some("A file"));

        let read = client.read_resource("file:///a.txt").await.unwrap();
        assert_eq!(read.contents.len(), 1);
        assert_eq!(read.contents[0].text.as_deref(), Some("hello"));

        assert_eq!(
            *requests.lock().await,
            vec![
                "resources/list".to_string(),
                "resources/read".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn test_unsupported_resources_surfaces_method_not_found() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = McpClient::from_transport(
            "fake",
            Box::new(UnsupportedResourcesTransport {
                requests: requests.clone(),
            }),
        );

        let list_err = client.list_resources().await.unwrap_err();
        assert!(matches!(
            list_err,
            McpError::JsonRpcError { code, .. } if code == METHOD_NOT_FOUND_CODE
        ));

        let read_err = client.read_resource("file:///x").await.unwrap_err();
        assert!(matches!(
            read_err,
            McpError::JsonRpcError { code, .. } if code == METHOD_NOT_FOUND_CODE
        ));

        assert_eq!(
            *requests.lock().await,
            vec![
                "resources/list".to_string(),
                "resources/read".to_string(),
            ]
        );
    }
}
