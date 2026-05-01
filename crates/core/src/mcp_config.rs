use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Transport type for an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportType {
    Stdio,
    Sse,
    Http,
    /// Catch-all for unsupported transport types.
    #[serde(untagged)]
    Unsupported(String),
}

impl Default for McpTransportType {
    fn default() -> Self {
        McpTransportType::Stdio
    }
}

impl fmt::Display for McpTransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpTransportType::Stdio => write!(f, "stdio"),
            McpTransportType::Sse => write!(f, "sse"),
            McpTransportType::Http => write!(f, "http"),
            McpTransportType::Unsupported(value) => write!(f, "{value}"),
        }
    }
}

/// Reconnect configuration for remote MCP transports.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpReconnectConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub initial_delay_ms: Option<u64>,
    #[serde(default)]
    pub max_delay_ms: Option<u64>,
}

/// Configuration for a single MCP server as defined in `settings.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Transport type.
    #[serde(rename = "type", default)]
    pub transport_type: McpTransportType,

    /// Executable command to start the MCP server.
    #[serde(default)]
    pub command: String,

    /// Command-line arguments for the server process.
    #[serde(default)]
    pub args: Vec<String>,

    /// Additional environment variables for the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory for the server process.
    #[serde(default)]
    pub cwd: Option<String>,

    /// Remote MCP endpoint URL for SSE and Streamable HTTP transports.
    #[serde(default)]
    pub url: Option<String>,

    /// Additional HTTP headers for remote MCP transports.
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Timeout for remote MCP transport operations.
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Reconnect behavior for remote MCP transports.
    #[serde(default)]
    pub reconnect: Option<McpReconnectConfig>,
}

/// A map of server name → server configuration, as it appears under `mcpServers`.
pub type McpServersConfig = HashMap<String, McpServerConfig>;

/// Runtime connection state of an MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerState {
    /// Server has been configured but not yet started.
    Pending,
    /// Server connected and initialized successfully.
    Connected,
    /// Remote server is disconnected and may reconnect later.
    Disconnected(String),
    /// Server failed during startup or initialization.
    Failed(String),
}

/// Runtime metadata for a single MCP server, including connection state
/// and discovered tools.
#[derive(Debug, Clone)]
pub struct McpServerStatus {
    /// The server name (key from `mcpServers` config).
    pub name: String,
    /// The configured transport type.
    pub transport_type: McpTransportType,
    /// Current connection state.
    pub state: McpServerState,
    /// Tools discovered via `tools/list` (empty if not connected).
    pub tools: Vec<McpToolInfo>,
}

/// Information about a single tool discovered from an MCP server.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    /// The tool name as reported by the MCP server.
    pub name: String,
    /// Human-readable description of the tool.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
}

impl McpServerConfig {
    /// Returns true if this server uses a supported transport type.
    pub fn is_supported_transport(&self) -> bool {
        matches!(
            self.transport_type,
            McpTransportType::Stdio | McpTransportType::Sse | McpTransportType::Http
        )
    }
}

/// Filter an `mcpServers` config map, keeping only servers with supported
/// transport types. Returns the filtered map and a list of (name, type)
/// pairs that were skipped.
pub fn filter_supported_servers(
    servers: &McpServersConfig,
) -> (McpServersConfig, Vec<(String, String)>) {
    let mut supported = McpServersConfig::new();
    let mut skipped = Vec::new();

    for (name, config) in servers {
        if config.is_supported_transport() {
            supported.insert(name.clone(), config.clone());
        } else {
            let type_str = match &config.transport_type {
                McpTransportType::Stdio => "stdio".to_string(),
                McpTransportType::Sse => "sse".to_string(),
                McpTransportType::Http => "http".to_string(),
                McpTransportType::Unsupported(t) => t.clone(),
            };
            skipped.push((name.clone(), type_str));
        }
    }

    (supported, skipped)
}

/// Merge two `mcpServers` maps. The `high` layer (e.g. project) overrides
/// the `low` layer (e.g. user) for the same server name.
pub fn merge_mcp_servers(high: &McpServersConfig, low: &McpServersConfig) -> McpServersConfig {
    let mut merged = low.clone();
    for (name, config) in high {
        merged.insert(name.clone(), config.clone());
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_stdio_server() {
        let json = r#"{
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@anthropic/mcp-server-filesystem"],
            "env": {"HOME": "/tmp"},
            "cwd": "/workspace"
        }"#;
        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.transport_type, McpTransportType::Stdio);
        assert_eq!(config.command, "npx");
        assert_eq!(config.args, vec!["-y", "@anthropic/mcp-server-filesystem"]);
        assert_eq!(config.env.get("HOME").unwrap(), "/tmp");
        assert_eq!(config.cwd.as_deref(), Some("/workspace"));
    }

    #[test]
    fn test_deserialize_minimal_stdio_server() {
        let json = r#"{"type": "stdio", "command": "npx"}"#;
        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.transport_type, McpTransportType::Stdio);
        assert_eq!(config.command, "npx");
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert!(config.cwd.is_none());
    }

    #[test]
    fn test_deserialize_unsupported_transport() {
        let json = r#"{"type": "websocket", "command": "some-server"}"#;
        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert!(!config.is_supported_transport());
        assert!(
            matches!(config.transport_type, McpTransportType::Unsupported(ref t) if t == "websocket")
        );
    }

    #[test]
    fn test_deserialize_sse_server() {
        let json = r#"{
            "type": "sse",
            "url": "https://example.test/mcp",
            "headers": {"Authorization": "Bearer token"},
            "timeout_ms": 5000,
            "reconnect": {"enabled": true, "initial_delay_ms": 100, "max_delay_ms": 2000}
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.transport_type, McpTransportType::Sse);
        assert!(config.is_supported_transport());
        assert_eq!(config.url.as_deref(), Some("https://example.test/mcp"));
        assert_eq!(
            config.headers.get("Authorization").map(String::as_str),
            Some("Bearer token")
        );
        assert_eq!(config.timeout_ms, Some(5000));
        let reconnect = config.reconnect.as_ref().unwrap();
        assert!(reconnect.enabled);
        assert_eq!(reconnect.initial_delay_ms, Some(100));
        assert_eq!(reconnect.max_delay_ms, Some(2000));
    }

    #[test]
    fn test_deserialize_http_server() {
        let json = r#"{"type": "http", "url": "https://example.test/mcp"}"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.transport_type, McpTransportType::Http);
        assert!(config.is_supported_transport());
        assert_eq!(config.url.as_deref(), Some("https://example.test/mcp"));
        assert!(config.headers.is_empty());
        assert!(config.reconnect.is_none());
    }

    #[test]
    fn test_filter_supported_servers() {
        let mut servers = McpServersConfig::new();
        servers.insert(
            "fs".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "npx".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                url: None,
                headers: HashMap::new(),
                timeout_ms: None,
                reconnect: None,
            },
        );
        servers.insert(
            "remote".into(),
            McpServerConfig {
                transport_type: McpTransportType::Unsupported("sse".into()),
                command: "http-server".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                url: None,
                headers: HashMap::new(),
                timeout_ms: None,
                reconnect: None,
            },
        );

        let (supported, skipped) = filter_supported_servers(&servers);
        assert_eq!(supported.len(), 1);
        assert!(supported.contains_key("fs"));
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0], ("remote".into(), "sse".into()));
    }

    #[test]
    fn test_merge_mcp_servers_different_names() {
        let mut low = McpServersConfig::new();
        low.insert(
            "github".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "gh-mcp".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                url: None,
                headers: HashMap::new(),
                timeout_ms: None,
                reconnect: None,
            },
        );
        let mut high = McpServersConfig::new();
        high.insert(
            "filesystem".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "fs-mcp".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                url: None,
                headers: HashMap::new(),
                timeout_ms: None,
                reconnect: None,
            },
        );

        let merged = merge_mcp_servers(&high, &low);
        assert_eq!(merged.len(), 2);
        assert!(merged.contains_key("github"));
        assert!(merged.contains_key("filesystem"));
    }

    #[test]
    fn test_merge_mcp_servers_same_name_high_wins() {
        let mut low = McpServersConfig::new();
        low.insert(
            "filesystem".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "a".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                url: None,
                headers: HashMap::new(),
                timeout_ms: None,
                reconnect: None,
            },
        );
        let mut high = McpServersConfig::new();
        high.insert(
            "filesystem".into(),
            McpServerConfig {
                transport_type: McpTransportType::Stdio,
                command: "b".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                url: None,
                headers: HashMap::new(),
                timeout_ms: None,
                reconnect: None,
            },
        );

        let merged = merge_mcp_servers(&high, &low);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged["filesystem"].command, "b");
    }

    #[test]
    fn test_default_transport_type_is_stdio() {
        let json = r#"{"command": "npx"}"#;
        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.transport_type, McpTransportType::Stdio);
        assert!(config.is_supported_transport());
    }

    #[test]
    fn test_remote_disconnected_state_is_distinct_from_failed() {
        let state = McpServerState::Disconnected("network error".to_string());

        assert_eq!(
            state,
            McpServerState::Disconnected("network error".to_string())
        );
    }
}
