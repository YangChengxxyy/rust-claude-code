use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rust_claude_mcp::jsonrpc::{JsonRpcNotification, JsonRpcRequest};
use tokio::sync::Mutex;

use super::error::LspError;
use super::language::{detect_language_from_path, discover_server_command, LspLanguage};
use super::protocol::LspRequest;
use super::transport::LspTransport;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LspSessionKey {
    pub language: LspLanguage,
    pub cwd: PathBuf,
}

pub struct LspManager {
    sessions: Mutex<HashMap<LspSessionKey, Arc<LspTransport>>>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub async fn ensure_session(&self, cwd: &Path, target: &Path) -> Result<LspSessionKey, LspError> {
        let language = detect_language_from_path(target)
            .ok_or_else(|| LspError::UnsupportedLanguage(target.display().to_string()))?;
        let key = LspSessionKey {
            language,
            cwd: cwd.to_path_buf(),
        };

        let mut sessions = self.sessions.lock().await;
        if !sessions.contains_key(&key) {
            let server = discover_server_command(language);
            let transport = LspTransport::start(&server, cwd)?;

            // LSP spec: send initialize request and wait for response.
            let init = LspRequest::initialize(cwd);
            let request = JsonRpcRequest::new(init.method, Some(init.params));
            transport.send_request(&request).await?;

            // LSP spec: send `initialized` notification after receiving initialize response.
            let initialized = JsonRpcNotification {
                jsonrpc: "2.0".into(),
                method: "initialized".into(),
                params: Some(serde_json::json!({})),
            };
            transport.send_notification(&initialized).await?;

            sessions.insert(key.clone(), Arc::new(transport));
        }
        Ok(key)
    }

    pub async fn request(&self, key: &LspSessionKey, request: LspRequest) -> Result<serde_json::Value, LspError> {
        // Clone the Arc to release the lock before sending the request,
        // so concurrent LSP requests are not serialized.
        let transport = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(key)
                .cloned()
                .ok_or_else(|| LspError::Protocol("LSP session not initialized".into()))?
        };
        let rpc = JsonRpcRequest::new(request.method, Some(request.params));
        transport.send_request(&rpc).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unsupported_language_errors() {
        let manager = LspManager::new();
        let result = manager
            .ensure_session(Path::new("/tmp"), Path::new("README.md"))
            .await;
        assert!(matches!(result, Err(LspError::UnsupportedLanguage(_))));
    }
}
