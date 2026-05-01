//! Stdio transport layer for MCP servers.
//!
//! Spawns a child process and communicates via Content-Length framed
//! JSON-RPC over stdin/stdout.

use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use rust_claude_core::mcp_config::McpServerConfig;

use crate::error::McpError;
use crate::jsonrpc::{
    self, check_response, parse_response, write_message, JsonRpcRequest, JsonRpcResponse,
};

#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    async fn send_request(&self, request: &JsonRpcRequest) -> Result<serde_json::Value, McpError>;
    async fn send_notification(&self, json: &[u8]) -> Result<(), McpError>;
    async fn shutdown(&mut self);
}

#[derive(Debug)]
pub struct SseTransport {
    url: String,
    client: reqwest::Client,
    timeout_ms: u64,
}

#[derive(Debug)]
pub struct HttpTransport {
    url: String,
    client: reqwest::Client,
    timeout_ms: u64,
}

impl HttpTransport {
    pub async fn connect(config: &McpServerConfig, timeout_ms: u64) -> Result<Self, McpError> {
        let url = config
            .url
            .clone()
            .filter(|url| !url.trim().is_empty())
            .ok_or_else(|| McpError::Protocol("HTTP MCP transport requires url".to_string()))?;

        Ok(Self {
            url,
            client: reqwest::Client::new(),
            timeout_ms,
        })
    }
}

#[async_trait::async_trait]
impl McpTransport for HttpTransport {
    async fn send_request(&self, request: &JsonRpcRequest) -> Result<serde_json::Value, McpError> {
        let request_id = request.id;
        let duration = Duration::from_millis(self.timeout_ms);
        let response = timeout(duration, async {
            self.client
                .post(&self.url)
                .json(request)
                .send()
                .await
                .map_err(|e| McpError::Protocol(format!("HTTP MCP request failed: {e}")))
        })
        .await
        .map_err(|_| McpError::Timeout)??;

        let status = response.status();
        if !status.is_success() {
            return Err(McpError::Protocol(format!(
                "HTTP MCP request failed with status {status}"
            )));
        }

        let body = timeout(duration, response.bytes())
            .await
            .map_err(|_| McpError::Timeout)?
            .map_err(|e| McpError::Protocol(format!("HTTP MCP response read failed: {e}")))?;
        parse_http_response(&body, request_id)
    }

    async fn send_notification(&self, json: &[u8]) -> Result<(), McpError> {
        let duration = Duration::from_millis(self.timeout_ms);
        let response = timeout(duration, async {
            self.client
                .post(&self.url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(json.to_vec())
                .send()
                .await
                .map_err(|e| McpError::Protocol(format!("HTTP MCP notification failed: {e}")))
        })
        .await
        .map_err(|_| McpError::Timeout)??;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(McpError::Protocol(format!(
                "HTTP MCP notification failed with status {}",
                response.status()
            )))
        }
    }

    async fn shutdown(&mut self) {}
}

impl SseTransport {
    pub async fn connect(config: &McpServerConfig, timeout_ms: u64) -> Result<Self, McpError> {
        let url = config
            .url
            .clone()
            .filter(|url| !url.trim().is_empty())
            .ok_or_else(|| McpError::Protocol("SSE MCP transport requires url".to_string()))?;

        Ok(Self {
            url,
            client: reqwest::Client::new(),
            timeout_ms,
        })
    }
}

#[async_trait::async_trait]
impl McpTransport for SseTransport {
    async fn send_request(&self, request: &JsonRpcRequest) -> Result<serde_json::Value, McpError> {
        let request_id = request.id;
        let duration = Duration::from_millis(self.timeout_ms);
        let response = timeout(duration, async {
            self.client
                .post(&self.url)
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .json(request)
                .send()
                .await
                .map_err(|e| McpError::Protocol(format!("SSE request failed: {e}")))
        })
        .await
        .map_err(|_| McpError::Timeout)??;

        let status = response.status();
        if !status.is_success() {
            return Err(McpError::Protocol(format!(
                "SSE request failed with status {status}"
            )));
        }

        let body = timeout(duration, response.text())
            .await
            .map_err(|_| McpError::Timeout)?
            .map_err(|e| McpError::Protocol(format!("SSE response read failed: {e}")))?;
        parse_sse_response(&body, request_id)
    }

    async fn send_notification(&self, _json: &[u8]) -> Result<(), McpError> {
        Ok(())
    }

    async fn shutdown(&mut self) {}
}

fn parse_sse_response(body: &str, request_id: u64) -> Result<serde_json::Value, McpError> {
    for event in body.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() {
            continue;
        }
        let response = parse_response(data.as_bytes())?;
        if let Some(response) = response {
            if response.id == Some(request_id) {
                return check_response(response);
            }
        }
    }

    Err(McpError::Protocol(format!(
        "no SSE response for request id {request_id}"
    )))
}

fn parse_http_response(body: &[u8], request_id: u64) -> Result<serde_json::Value, McpError> {
    match parse_response(body)? {
        Some(response) if response.id == Some(request_id) => check_response(response),
        Some(response) => Err(McpError::Protocol(format!(
            "unexpected JSON-RPC response id {:?}, expected {request_id}",
            response.id
        ))),
        None => Err(McpError::Protocol(
            "HTTP MCP response did not contain a JSON-RPC response".to_string(),
        )),
    }
}

/// Default timeout for MCP operations (30 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// A stdio transport connection to an MCP server process.
#[derive(Debug)]
pub struct StdioTransport {
    /// The child process handle.
    child: Child,
    /// Buffered reader for the child's stdout.
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    /// Writer for the child's stdin.
    pub(crate) stdin: Mutex<tokio::process::ChildStdin>,
    /// Timeout for individual operations.
    timeout_ms: u64,
}

impl StdioTransport {
    /// Start a new MCP server process and return a transport handle.
    pub fn start(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<Self, McpError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| {
            McpError::ProcessStartFailed(format!("failed to start '{}': {}", command, e))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::ProcessStartFailed("no stdout handle".into()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::ProcessStartFailed("no stdin handle".into()))?;

        Ok(StdioTransport {
            child,
            stdout: Mutex::new(BufReader::new(stdout)),
            stdin: Mutex::new(stdin),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        })
    }

    /// Set the timeout for operations.
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    async fn send_request_inner(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<serde_json::Value, McpError> {
        let request_id = request.id;
        let json = serde_json::to_vec(request)
            .map_err(|e| McpError::InvalidJson(format!("failed to serialize request: {e}")))?;

        // Send the request
        {
            let mut stdin = self.stdin.lock().await;
            write_message(&mut *stdin, &json).await?;
        }

        // Read responses until we get one matching our request ID
        let duration = Duration::from_millis(self.timeout_ms);
        let response = timeout(duration, self.read_response(request_id))
            .await
            .map_err(|_| McpError::Timeout)??;

        check_response(response)
    }

    /// Read from stdout until we get a response matching the given request ID.
    /// Notifications (messages without an ID) are silently skipped.
    async fn read_response(&self, request_id: u64) -> Result<JsonRpcResponse, McpError> {
        let mut stdout = self.stdout.lock().await;
        loop {
            let body = jsonrpc::read_message(&mut *stdout).await?;
            match parse_response(&body)? {
                Some(response) if response.id == Some(request_id) => return Ok(response),
                Some(_other) => {
                    // Response for a different request ID — skip
                    // (shouldn't normally happen in a single-threaded request model)
                    continue;
                }
                None => {
                    // Notification — skip and keep reading
                    continue;
                }
            }
        }
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    /// Send a JSON-RPC request and wait for a matching response.
    /// Notifications from the server are silently skipped.
    async fn send_request(&self, request: &JsonRpcRequest) -> Result<serde_json::Value, McpError> {
        self.send_request_inner(request).await
    }

    async fn send_notification(&self, json: &[u8]) -> Result<(), McpError> {
        let mut stdin = self.stdin.lock().await;
        write_message(&mut *stdin, json).await
    }

    /// Kill the child process.
    async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }
}

impl StdioTransport {
    /// Send a JSON-RPC request and wait for a matching response.
    /// Notifications from the server are silently skipped.
    pub async fn send_request(&self, request: &JsonRpcRequest) -> Result<serde_json::Value, McpError> {
        self.send_request_inner(request).await
    }

    pub async fn send_notification(&self, json: &[u8]) -> Result<(), McpError> {
        <Self as McpTransport>::send_notification(self, json).await
    }

    /// Kill the child process.
    pub async fn shutdown(&mut self) {
        <Self as McpTransport>::shutdown(self).await;
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Best-effort kill on drop. We can't await here, so use start_kill.
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::mcp_config::{McpServerConfig, McpTransportType};

    fn assert_transport_trait<T: McpTransport>() {}

    #[test]
    fn test_stdio_transport_implements_transport_trait() {
        assert_transport_trait::<StdioTransport>();
    }

    #[tokio::test]
    async fn test_sse_transport_requires_url() {
        let config = McpServerConfig {
            transport_type: McpTransportType::Sse,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            url: None,
            headers: HashMap::new(),
            timeout_ms: None,
            reconnect: None,
        };

        let error = SseTransport::connect(&config, 1000).await.unwrap_err();

        assert!(matches!(error, McpError::Protocol(message) if message.contains("url")));
    }

    #[tokio::test]
    async fn test_http_transport_requires_url() {
        let config = McpServerConfig {
            transport_type: McpTransportType::Http,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            url: None,
            headers: HashMap::new(),
            timeout_ms: None,
            reconnect: None,
        };

        let error = HttpTransport::connect(&config, 1000).await.unwrap_err();

        assert!(matches!(error, McpError::Protocol(message) if message.contains("url")));
    }

    #[test]
    fn test_parse_sse_response_data_event() {
        let value = parse_sse_response(
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n",
            1,
        )
        .unwrap();

        assert_eq!(value, serde_json::json!({"ok": true}));
    }

    #[test]
    fn test_parse_http_response_json_rpc_result() {
        let value = parse_http_response(
            br#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
            1,
        )
        .unwrap();

        assert_eq!(value, serde_json::json!({"ok": true}));
    }

    #[tokio::test]
    async fn test_start_nonexistent_command() {
        let result = StdioTransport::start(
            "/nonexistent/binary/that/does/not/exist",
            &[],
            &HashMap::new(),
            None,
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpError::ProcessStartFailed(_)
        ));
    }

    #[tokio::test]
    async fn test_start_and_kill_process() {
        // Use a simple long-running process
        let mut transport = StdioTransport::start("cat", &[], &HashMap::new(), None).unwrap();

        assert!(transport.is_alive());
        transport.shutdown().await;
        // After shutdown, process should be dead
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!transport.is_alive());
    }

    #[tokio::test]
    async fn test_timeout_on_no_response() {
        // Start `sleep` which reads nothing and writes nothing.
        // The request will be sent but no response ever comes back.
        let transport = StdioTransport::start("sleep", &["60".to_string()], &HashMap::new(), None)
            .unwrap()
            .with_timeout_ms(200);

        let request = JsonRpcRequest::new("test/method", None);
        let result = transport.send_request(&request).await;

        assert!(matches!(result, Err(McpError::Timeout)));
    }
}
