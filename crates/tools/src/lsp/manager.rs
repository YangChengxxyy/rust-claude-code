use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rust_claude_mcp::jsonrpc::JsonRpcRequest;
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
    sessions: Mutex<HashMap<LspSessionKey, LspTransport>>,
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
            let init = LspRequest::initialize(cwd);
            let request = JsonRpcRequest::new(init.method, Some(init.params));
            transport.send_request(&request).await?;
            sessions.insert(key.clone(), transport);
        }
        Ok(key)
    }

    pub async fn request(&self, key: &LspSessionKey, request: LspRequest) -> Result<serde_json::Value, LspError> {
        let sessions = self.sessions.lock().await;
        let transport = sessions
            .get(key)
            .ok_or_else(|| LspError::Protocol("LSP session not initialized".into()))?;
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
