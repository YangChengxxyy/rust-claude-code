use std::time::Duration;

use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    Client, Method, RequestBuilder, StatusCode,
};

use crate::error::ApiError;
use crate::streaming::{stream_events_from_response, MessageStream};
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
        self.send_json(Method::POST, self.messages_endpoint(), request)
            .await
    }

    pub async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError> {
        let request = request.clone().with_stream(true);
        let response = self
            .request(Method::POST, self.messages_endpoint())?
            .json(&request)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(map_reqwest_error)?;
            return Err(map_error_response(status, &body));
        }

        Ok(stream_events_from_response(response))
    }

    fn messages_endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn request(&self, method: Method, url: String) -> Result<RequestBuilder, ApiError> {
        Ok(self
            .http_client
            .request(method, url)
            .headers(self.default_headers()?))
    }

    fn default_headers(&self) -> Result<HeaderMap, ApiError> {
        let mut headers = HeaderMap::new();
        let api_key = HeaderValue::from_str(&self.api_key)
            .map_err(|error| ApiError::Auth(format!("invalid API key header value: {error}")))?;
        headers.insert("x-api-key", api_key);
        headers.insert(
            "anthropic-version",
            HeaderValue::from_str(&self.anthropic_version)
                .map_err(|error| ApiError::Api {
                    status: 0,
                    message: format!("invalid anthropic-version header value: {error}"),
                })?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    async fn parse_json_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();
        let body = response.text().await.map_err(map_reqwest_error)?;

        if status.is_success() {
            return serde_json::from_str(&body).map_err(ApiError::from);
        }

        Err(map_error_response(status, &body))
    }

    async fn send_json<TRequest, TResponse>(
        &self,
        method: Method,
        url: String,
        request: &TRequest,
    ) -> Result<TResponse, ApiError>
    where
        TRequest: serde::Serialize + ?Sized,
        TResponse: serde::de::DeserializeOwned,
    {
        let response = self
            .request(method, url)?
            .json(request)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        self.parse_json_response(response).await
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
    fn test_default_headers_include_auth_version_and_content_type() {
        let client = AnthropicClient::new("test-key")
            .unwrap()
            .with_version("2024-01-01");

        let headers = client.default_headers().unwrap();

        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2024-01-01");
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
    }

    #[test]
    fn test_default_headers_reject_invalid_api_key_value() {
        let client = AnthropicClient::new("test-key")
            .unwrap()
            .with_version("2024-01-01");
        let mut client = client;
        client.api_key = "bad\nkey".to_string();

        let error = client.default_headers().unwrap_err();
        assert!(matches!(error, ApiError::Auth(message) if message.contains("invalid API key header value")));
    }

    #[test]
    fn test_default_headers_reject_invalid_version_value() {
        let client = AnthropicClient::new("test-key").unwrap();
        let mut client = client;
        client.anthropic_version = "2024-01-01\n".to_string();

        let error = client.default_headers().unwrap_err();
        assert!(matches!(error, ApiError::Api { status: 0, message } if message.contains("invalid anthropic-version header value")));
    }

    #[test]
    fn test_request_uses_messages_endpoint() {
        let client = AnthropicClient::new("test-key")
            .unwrap()
            .with_base_url("https://example.com");

        let request = client
            .http_client
            .request(Method::POST, client.messages_endpoint())
            .headers(client.default_headers().unwrap())
            .build()
            .expect("request should build");

        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.url().as_str(), "https://example.com/v1/messages");
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
