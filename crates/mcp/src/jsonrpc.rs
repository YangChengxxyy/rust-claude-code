//! JSON-RPC 2.0 message types and Content-Length framing for MCP stdio transport.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::error::McpError;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Generate a unique request ID.
pub fn next_request_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: next_request_id(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification (no `id` field, server → client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Write a JSON-RPC message with `Content-Length` framing to the given writer.
pub async fn write_message<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &[u8],
) -> Result<(), McpError> {
    let header = format!("Content-Length: {}\r\n\r\n", message.len());
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| McpError::Io(e))?;
    writer
        .write_all(message)
        .await
        .map_err(|e| McpError::Io(e))?;
    writer.flush().await.map_err(|e| McpError::Io(e))?;
    Ok(())
}

/// Read a single JSON-RPC message with `Content-Length` framing from the given reader.
/// Returns the raw JSON bytes of the body.
pub async fn read_message<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> Result<Vec<u8>, McpError> {
    let mut content_length: Option<usize> = None;

    // Read headers until empty line
    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .await
            .map_err(|e| McpError::Io(e))?;

        if bytes_read == 0 {
            return Err(McpError::ProcessExited);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let len_str = value.trim();
            content_length = Some(len_str.parse::<usize>().map_err(|_| {
                McpError::InvalidFraming(format!("invalid Content-Length: {len_str}"))
            })?);
        }
        // Ignore other headers (e.g. Content-Type)
    }

    let length = content_length
        .ok_or_else(|| McpError::InvalidFraming("missing Content-Length header".into()))?;

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| McpError::Io(e))?;

    Ok(body)
}

/// Parse a JSON-RPC response from raw bytes. Handles both response and
/// notification messages from the server. If the message is a notification
/// (no `id`), returns `None`.
pub fn parse_response(body: &[u8]) -> Result<Option<JsonRpcResponse>, McpError> {
    // First try to parse as a generic JSON value to inspect structure
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| McpError::InvalidJson(format!("malformed JSON: {e}")))?;

    // If there's no "id" field, it's a notification — skip it
    if value.get("id").is_none() || value.get("id") == Some(&serde_json::Value::Null) {
        return Ok(None);
    }

    let response: JsonRpcResponse = serde_json::from_value(value)
        .map_err(|e| McpError::InvalidJson(format!("invalid JSON-RPC response: {e}")))?;

    Ok(Some(response))
}

/// Check a JSON-RPC response for errors. Returns the result value on success.
pub fn check_response(response: JsonRpcResponse) -> Result<serde_json::Value, McpError> {
    if let Some(error) = response.error {
        return Err(McpError::JsonRpcError {
            code: error.code,
            message: error.message,
        });
    }
    Ok(response.result.unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn test_write_and_read_message() {
        let message = b"hello world";
        let mut buffer = Vec::new();
        write_message(&mut buffer, message).await.unwrap();

        let expected = b"Content-Length: 11\r\n\r\nhello world";
        assert_eq!(buffer, expected);

        let mut reader = BufReader::new(&buffer[..]);
        let body = read_message(&mut reader).await.unwrap();
        assert_eq!(body, b"hello world");
    }

    #[tokio::test]
    async fn test_write_and_read_json_rpc() {
        let request = JsonRpcRequest::new("test/method", Some(serde_json::json!({"key": "value"})));
        let json = serde_json::to_vec(&request).unwrap();

        let mut buffer = Vec::new();
        write_message(&mut buffer, &json).await.unwrap();

        let mut reader = BufReader::new(&buffer[..]);
        let body = read_message(&mut reader).await.unwrap();

        let parsed: JsonRpcRequest = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.method, "test/method");
        assert_eq!(parsed.jsonrpc, "2.0");
    }

    #[tokio::test]
    async fn test_read_message_missing_content_length() {
        let data = b"\r\nhello";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(McpError::InvalidFraming(_))));
    }

    #[tokio::test]
    async fn test_read_message_invalid_content_length() {
        let data = b"Content-Length: abc\r\n\r\nhello";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(McpError::InvalidFraming(_))));
    }

    #[tokio::test]
    async fn test_read_message_eof() {
        let data = b"";
        let mut reader = BufReader::new(&data[..]);
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(McpError::ProcessExited)));
    }

    #[test]
    fn test_parse_response_success() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"status":"ok"}}"#;
        let response = parse_response(json.as_bytes()).unwrap().unwrap();
        assert_eq!(response.id, Some(1));
        assert!(response.error.is_none());
        assert_eq!(
            response.result.unwrap(),
            serde_json::json!({"status": "ok"})
        );
    }

    #[test]
    fn test_parse_response_error() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let response = parse_response(json.as_bytes()).unwrap().unwrap();
        let err = response.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn test_parse_response_notification_is_none() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/message","params":{}}"#;
        let result = parse_response(json.as_bytes()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_response_invalid_json() {
        let result = parse_response(b"not json");
        assert!(matches!(result, Err(McpError::InvalidJson(_))));
    }

    #[test]
    fn test_check_response_success() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(1),
            result: Some(serde_json::json!({"data": "test"})),
            error: None,
        };
        let result = check_response(response).unwrap();
        assert_eq!(result, serde_json::json!({"data": "test"}));
    }

    #[test]
    fn test_check_response_error() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(1),
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request".into(),
                data: None,
            }),
        };
        let result = check_response(response);
        assert!(matches!(
            result,
            Err(McpError::JsonRpcError { code: -32600, .. })
        ));
    }

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: 42,
            method: "initialize".into(),
            params: Some(serde_json::json!({})),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[tokio::test]
    async fn test_multiple_messages_in_stream() {
        let msg1 = b"first message";
        let msg2 = b"second message";

        let mut buffer = Vec::new();
        write_message(&mut buffer, msg1).await.unwrap();
        write_message(&mut buffer, msg2).await.unwrap();

        let mut reader = BufReader::new(&buffer[..]);
        let body1 = read_message(&mut reader).await.unwrap();
        let body2 = read_message(&mut reader).await.unwrap();

        assert_eq!(body1, b"first message");
        assert_eq!(body2, b"second message");
    }
}
