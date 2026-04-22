/// Errors that can occur during MCP operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("process start failed: {0}")]
    ProcessStartFailed(String),

    #[error("process exited unexpectedly")]
    ProcessExited,

    #[error("timeout waiting for response")]
    Timeout,

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("invalid JSON: {0}")]
    InvalidJson(String),

    #[error("invalid framing: {0}")]
    InvalidFraming(String),

    #[error("JSON-RPC error (code {code}): {message}")]
    JsonRpcError { code: i64, message: String },

    #[error("server not connected: {0}")]
    ServerNotConnected(String),

    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
