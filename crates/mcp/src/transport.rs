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

use crate::error::McpError;
use crate::jsonrpc::{
    self, check_response, parse_response, write_message, JsonRpcRequest, JsonRpcResponse,
};

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
            McpError::ProcessStartFailed(format!(
                "failed to start '{}': {}",
                command, e
            ))
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

    /// Send a JSON-RPC request and wait for a matching response.
    /// Notifications from the server are silently skipped.
    pub async fn send_request(
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

    /// Kill the child process.
    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
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

    #[tokio::test]
    async fn test_start_nonexistent_command() {
        let result = StdioTransport::start(
            "/nonexistent/binary/that/does/not/exist",
            &[],
            &HashMap::new(),
            None,
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::ProcessStartFailed(_)));
    }

    #[tokio::test]
    async fn test_start_and_kill_process() {
        // Use a simple long-running process
        let mut transport = StdioTransport::start(
            "cat",
            &[],
            &HashMap::new(),
            None,
        )
        .unwrap();

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
        let transport = StdioTransport::start(
            "sleep",
            &["60".to_string()],
            &HashMap::new(),
            None,
        )
        .unwrap()
        .with_timeout_ms(200);

        let request = JsonRpcRequest::new("test/method", None);
        let result = transport.send_request(&request).await;

        assert!(matches!(result, Err(McpError::Timeout)));
    }
}
