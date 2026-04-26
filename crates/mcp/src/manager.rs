//! MCP server lifecycle manager.
//!
//! `McpManager` starts configured MCP servers, discovers their tools,
//! and exposes a unified interface for tool lookup and invocation.

use rust_claude_core::mcp_config::{
    filter_supported_servers, McpServerConfig, McpServerState, McpServerStatus, McpServersConfig,
    McpToolInfo, McpTransportType,
};
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::error::McpError;
use crate::protocol::{McpClient, ToolCallResult};

/// Configuration for the MCP manager.
#[derive(Debug, Clone)]
pub struct McpManagerConfig {
    /// Timeout for individual server initialization (milliseconds).
    pub init_timeout_ms: u64,
}

impl Default for McpManagerConfig {
    fn default() -> Self {
        McpManagerConfig {
            init_timeout_ms: 30_000,
        }
    }
}

/// A connected MCP server with its discovered tools.
struct ConnectedServer {
    client: McpClient,
    #[allow(dead_code)]
    tools: Vec<McpToolInfo>,
}

/// Manages multiple MCP server connections and provides a unified
/// interface for tool discovery and invocation.
pub struct McpManager {
    /// Connected servers keyed by server name.
    servers: HashMap<String, Mutex<ConnectedServer>>,
    /// Status snapshots for all configured servers (connected + failed).
    statuses: Vec<McpServerStatus>,
    /// Lookup: fully qualified tool name → (server_name, mcp_tool_name).
    tool_index: HashMap<String, (String, String)>,
    /// Tool info for all discovered tools, keyed by fully qualified name.
    tool_infos: HashMap<String, McpToolInfo>,
}

impl McpManager {
    /// Start all configured MCP servers and discover their tools.
    ///
    /// Servers with unsupported transport types are skipped with a warning.
    /// Individual server failures do not block the overall startup.
    pub async fn start(servers_config: &McpServersConfig, config: &McpManagerConfig) -> Self {
        let (supported, skipped) = filter_supported_servers(servers_config);

        let mut connected_servers = HashMap::new();
        let mut statuses = Vec::new();
        let mut tool_index = HashMap::new();
        let mut tool_infos = HashMap::new();

        // Record skipped servers
        for (name, transport_type) in &skipped {
            eprintln!(
                "MCP: skipping server '{}' (unsupported transport: {})",
                name, transport_type
            );
            statuses.push(McpServerStatus {
                name: name.clone(),
                transport_type: McpTransportType::Unsupported(transport_type.clone()),
                state: McpServerState::Failed(format!(
                    "unsupported transport type: {}",
                    transport_type
                )),
                tools: vec![],
            });
        }

        // Start supported servers
        for (name, server_config) in &supported {
            match Self::start_server(name, server_config, config).await {
                Ok((client, tools)) => {
                    // Build tool index
                    for tool in &tools {
                        let qualified_name = format!("mcp__{}__{}", name, tool.name);
                        tool_index
                            .insert(qualified_name.clone(), (name.clone(), tool.name.clone()));
                        tool_infos.insert(qualified_name, tool.clone());
                    }

                    statuses.push(McpServerStatus {
                        name: name.clone(),
                        transport_type: McpTransportType::Stdio,
                        state: McpServerState::Connected,
                        tools: tools.clone(),
                    });

                    connected_servers
                        .insert(name.clone(), Mutex::new(ConnectedServer { client, tools }));
                }
                Err(e) => {
                    eprintln!("MCP: server '{}' failed to start: {}", name, e);
                    statuses.push(McpServerStatus {
                        name: name.clone(),
                        transport_type: McpTransportType::Stdio,
                        state: McpServerState::Failed(e.to_string()),
                        tools: vec![],
                    });
                }
            }
        }

        McpManager {
            servers: connected_servers,
            statuses,
            tool_index,
            tool_infos,
        }
    }

    /// Start a single server: connect, initialize, and list tools.
    async fn start_server(
        name: &str,
        config: &McpServerConfig,
        manager_config: &McpManagerConfig,
    ) -> Result<(McpClient, Vec<McpToolInfo>), McpError> {
        let client =
            McpClient::connect_with_timeout(name, config, manager_config.init_timeout_ms).await?;

        let tools = client.list_tools().await?;
        Ok((client, tools))
    }

    /// Create an empty McpManager (no servers configured).
    pub fn empty() -> Self {
        McpManager {
            servers: HashMap::new(),
            statuses: vec![],
            tool_index: HashMap::new(),
            tool_infos: HashMap::new(),
        }
    }

    /// Get the status of all configured servers.
    pub fn server_statuses(&self) -> &[McpServerStatus] {
        &self.statuses
    }

    /// Get all discovered tool infos with their fully qualified names.
    pub fn discovered_tools(&self) -> Vec<(String, &McpToolInfo)> {
        self.tool_infos
            .iter()
            .map(|(name, info)| (name.clone(), info))
            .collect()
    }

    /// Look up a tool by its fully qualified name (`mcp__<server>__<tool>`).
    pub fn get_tool_info(&self, qualified_name: &str) -> Option<&McpToolInfo> {
        self.tool_infos.get(qualified_name)
    }

    /// Call a tool by its fully qualified name.
    pub async fn call_tool(
        &self,
        qualified_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult, McpError> {
        let (server_name, tool_name) = self
            .tool_index
            .get(qualified_name)
            .ok_or_else(|| McpError::ToolNotFound(qualified_name.to_string()))?;

        let server = self
            .servers
            .get(server_name)
            .ok_or_else(|| McpError::ServerNotConnected(server_name.clone()))?;

        let server_guard = server.lock().await;
        server_guard.client.call_tool(tool_name, arguments).await
    }

    /// Shutdown all connected servers.
    pub async fn shutdown(&self) {
        for (_, server) in &self.servers {
            let mut server_guard = server.lock().await;
            server_guard.client.shutdown().await;
        }
    }

    /// Returns the number of connected servers.
    pub fn connected_count(&self) -> usize {
        self.servers.len()
    }

    /// Returns the total number of discovered tools.
    pub fn tool_count(&self) -> usize {
        self.tool_infos.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_manager() {
        let manager = McpManager::empty();
        assert_eq!(manager.connected_count(), 0);
        assert_eq!(manager.tool_count(), 0);
        assert!(manager.server_statuses().is_empty());
        assert!(manager.discovered_tools().is_empty());
    }

    #[tokio::test]
    async fn test_start_with_empty_config() {
        let config = McpServersConfig::new();
        let manager = McpManager::start(&config, &McpManagerConfig::default()).await;
        assert_eq!(manager.connected_count(), 0);
        assert_eq!(manager.tool_count(), 0);
    }

    #[tokio::test]
    async fn test_start_with_nonexistent_command() {
        let mut config = McpServersConfig::new();
        config.insert(
            "bad-server".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "/nonexistent/binary".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
            },
        );

        let manager = McpManager::start(&config, &McpManagerConfig::default()).await;
        assert_eq!(manager.connected_count(), 0);

        // Server should be recorded as failed
        let statuses = manager.server_statuses();
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0].state, McpServerState::Failed(_)));
    }

    #[tokio::test]
    async fn test_start_with_unsupported_transport() {
        let mut config = McpServersConfig::new();
        config.insert(
            "sse-server".into(),
            McpServerConfig {
                transport_type: McpTransportType::Unsupported("sse".into()),
                command: "http-server".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
            },
        );

        let manager = McpManager::start(&config, &McpManagerConfig::default()).await;
        assert_eq!(manager.connected_count(), 0);

        let statuses = manager.server_statuses();
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0].state, McpServerState::Failed(_)));
    }

    #[tokio::test]
    async fn test_failure_isolation_bad_server_does_not_block() {
        let mut config = McpServersConfig::new();
        // One bad server
        config.insert(
            "bad".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "/nonexistent/binary".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
            },
        );
        // Another bad server
        config.insert(
            "also-bad".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "/another/nonexistent/binary".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
            },
        );

        // Should not panic; both fail gracefully
        let manager = McpManager::start(&config, &McpManagerConfig::default()).await;
        assert_eq!(manager.connected_count(), 0);
        assert_eq!(manager.server_statuses().len(), 2);
    }

    #[tokio::test]
    async fn test_call_tool_not_found() {
        let manager = McpManager::empty();
        let result = manager
            .call_tool("mcp__unknown__tool", serde_json::json!({}))
            .await;
        assert!(matches!(result, Err(McpError::ToolNotFound(_))));
    }
}
