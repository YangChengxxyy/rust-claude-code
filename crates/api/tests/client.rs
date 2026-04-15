use rust_claude_api::{AnthropicClient, ApiContent, ApiError, CreateMessageRequest};

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY"]
async fn create_message_with_real_api_key() {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set for integration tests");
    let client = AnthropicClient::new(api_key).expect("client should initialize");
    let request = CreateMessageRequest::new(
        "claude-sonnet-4-20250514",
        vec![rust_claude_api::ApiMessage::user("Reply with exactly: pong")],
    )
    .with_max_tokens(32);

    let response = client
        .create_message(&request)
        .await
        .expect("request should succeed");

    assert_eq!(response.role, rust_claude_core::message::Role::Assistant);
    assert!(response.content.iter().any(|block| match block {
        rust_claude_core::message::ContentBlock::Text { text } => !text.trim().is_empty(),
        rust_claude_core::message::ContentBlock::ToolUse { .. }
        | rust_claude_core::message::ContentBlock::ToolResult { .. }
        | rust_claude_core::message::ContentBlock::Thinking { .. } => true,
    }));
}

#[test]
fn api_message_user_helper_uses_text_content() {
    let message = rust_claude_api::ApiMessage::user("hello");
    assert!(matches!(message.content, ApiContent::Text(text) if text == "hello"));
}

#[test]
fn api_message_assistant_helper_uses_text_content() {
    let message = rust_claude_api::ApiMessage::assistant("hello");
    assert!(matches!(message.content, ApiContent::Text(text) if text == "hello"));
}

#[test]
fn anthropic_client_rejects_blank_api_key() {
    let error = AnthropicClient::new("").expect_err("blank key should fail");
    assert!(matches!(error, ApiError::Auth(message) if message.contains("cannot be empty")));
}
