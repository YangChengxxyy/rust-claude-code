use std::time::Duration;

use reqwest::{Client, StatusCode};

use crate::error::ApiError;
use crate::types::{ApiErrorResponse, CreateMessageRequest, CreateMessageResponse};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_VERSION: &str = "2023-06-01";
const DEFAULT_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    http_client: Client,
    api_key: String,
    base_url: String,
    anthropic_version: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, ApiError> {
        Self::from_http_client(api_key, Client::builder().timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)).build()?)
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = normalize_base_url(base_url.into());
        self
    }

    pub fn with_version(mut self, anthropic_version: impl Into<String>) -> Self {
        self.anthropic_version = anthropic_version.into();
        self
    }

    pub fn from_http_client(
        api_key: impl Into<String>,
        http_client: Client,
    ) -> Result<Self, ApiError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(ApiError::Auth("API key cannot be empty".to_string()));
        }

        Ok(Self {
            http_client,
            api_key,
            base_url: DEFAULT_BASE_URL.to_string(),
            anthropic_version: DEFAULT_VERSION.to_string(),
        })
    }

    pub async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError> {
        let response = self
            .http_client
            .post(self.messages_endpoint())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.anthropic_version)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(request)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let body = response.text().await.map_err(map_reqwest_error)?;

        if status.is_success() {
            return serde_json::from_str(&body).map_err(ApiError::from);
        }

        Err(map_error_response(status, &body))
    }

    fn messages_endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }
}

fn normalize_base_url(base_url: String) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn map_reqwest_error(error: reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::Timeout
    } else if error.is_connect() {
        ApiError::Connection(error.to_string())
    } else {
        ApiError::Http(error)
    }
}

fn map_error_response(status: StatusCode, body: &str) -> ApiError {
    let message = match serde_json::from_str::<ApiErrorResponse>(body) {
        Ok(error_response) => error_response.error.message,
        Err(_) => body.trim().to_string(),
    };

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ApiError::Auth(message),
        StatusCode::TOO_MANY_REQUESTS => ApiError::RateLimited(message),
        _ => ApiError::Api {
            status: status.as_u16(),
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_api_key_is_rejected() {
        let client = AnthropicClient::new("   ");
        assert!(matches!(client, Err(ApiError::Auth(_))));
    }

    #[test]
    fn test_with_base_url_strips_trailing_slash() {
        let client = AnthropicClient::new("test-key")
            .unwrap()
            .with_base_url("https://example.com/");

        assert_eq!(client.messages_endpoint(), "https://example.com/v1/messages");
    }

    #[test]
    fn test_error_response_maps_auth_status() {
        let error = map_error_response(
            StatusCode::UNAUTHORIZED,
            r#"{"error":{"type":"authentication_error","message":"invalid key"}}"#,
        );

        assert!(matches!(error, ApiError::Auth(message) if message == "invalid key"));
    }

    #[test]
    fn test_error_response_maps_rate_limit_status() {
        let error = map_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#,
        );

        assert!(matches!(error, ApiError::RateLimited(message) if message == "slow down"));
    }

    #[test]
    fn test_error_response_falls_back_to_raw_body() {
        let error = map_error_response(StatusCode::BAD_REQUEST, "bad request");

        assert!(matches!(error, ApiError::Api { status: 400, message } if message == "bad request"));
    }
}
