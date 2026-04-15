use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("SSE stream error: {0}")]
    Stream(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Timeout")]
    Timeout,
}
