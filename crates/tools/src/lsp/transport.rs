use std::path::Path;
use std::process::Stdio;

use rust_claude_mcp::jsonrpc::{check_response, parse_response, write_message, JsonRpcRequest, JsonRpcNotification};
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

use super::error::LspError;
use super::language::ServerCommand;

const DEFAULT_TIMEOUT_MS: u64 = 20_000;

#[derive(Debug)]
pub struct LspTransport {
    child: Child,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    stdin: Mutex<tokio::process::ChildStdin>,
    timeout_ms: u64,
}

impl LspTransport {
    pub fn start(server: &ServerCommand, cwd: &Path) -> Result<Self, LspError> {
        let mut cmd = Command::new(&server.command);
        cmd.args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(cwd);

        let mut child = cmd
            .spawn()
            .map_err(|e| LspError::ServerStart(format!("{}: {}", server.command, e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::ServerStart("missing stdout".into()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::ServerStart("missing stdin".into()))?;

        Ok(Self {
            child,
            stdout: Mutex::new(BufReader::new(stdout)),
            stdin: Mutex::new(stdin),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        })
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub async fn send_request(&self, request: &JsonRpcRequest) -> Result<serde_json::Value, LspError> {
        let request_id = request.id;
        let body = serde_json::to_vec(request)?;

        {
            let mut stdin = self.stdin.lock().await;
            write_message(&mut *stdin, &body)
                .await
                .map_err(|e| LspError::Protocol(e.to_string()))?;
        }

        timeout(Duration::from_millis(self.timeout_ms), async {
            let mut stdout = self.stdout.lock().await;
            loop {
                let body = rust_claude_mcp::jsonrpc::read_message(&mut *stdout)
                    .await
                    .map_err(|e| LspError::Protocol(e.to_string()))?;
                match parse_response(&body).map_err(|e| LspError::InvalidResponse(e.to_string()))? {
                    Some(response) if response.id == Some(request_id) => {
                        return check_response(response)
                            .map_err(|e| LspError::Protocol(e.to_string()));
                    }
                    _ => continue,
                }
            }
        })
        .await
        .map_err(|_| LspError::Timeout)?
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    pub async fn send_notification(&self, notification: &JsonRpcNotification) -> Result<(), LspError> {
        let body = serde_json::to_vec(notification)?;
        let mut stdin = self.stdin.lock().await;
        write_message(&mut *stdin, &body)
            .await
            .map_err(|e| LspError::Protocol(e.to_string()))
    }

    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }
}

impl Drop for LspTransport {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_nonexistent_server_fails() {
        let server = ServerCommand {
            command: "/nonexistent/lsp".into(),
            args: vec![],
        };
        let result = LspTransport::start(&server, Path::new("/tmp"));
        assert!(matches!(result, Err(LspError::ServerStart(_))));
    }
}
