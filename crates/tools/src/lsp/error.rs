#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("unsupported language for path: {0}")]
    UnsupportedLanguage(String),

    #[error("failed to start language server: {0}")]
    ServerStart(String),

    #[error("language server protocol error: {0}")]
    Protocol(String),

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("request timed out")]
    Timeout,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
