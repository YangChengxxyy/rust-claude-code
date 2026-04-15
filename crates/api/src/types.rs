use serde::{Deserialize, Serialize};

use rust_claude_core::message::{ContentBlock, Role, StopReason, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: Role,
    pub content: ApiContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

impl ApiTool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        ApiTool {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMessageResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub model: String,
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

impl CreateMessageRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ApiMessage>) -> Self {
        CreateMessageRequest {
            model: model.into(),
            messages,
            system: None,
            max_tokens: None,
            stream: None,
            tools: None,
            stop_sequences: None,
            temperature: None,
        }
    }

    pub fn with_system(mut self, system: impl Into<SystemPrompt>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    pub fn with_tools(mut self, tools: Vec<ApiTool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }
}

impl From<&rust_claude_core::message::Message> for ApiMessage {
    fn from(msg: &rust_claude_core::message::Message) -> Self {
        ApiMessage {
            role: msg.role.clone(),
            content: ApiContent::Blocks(msg.content.clone()),
        }
    }
}

impl From<String> for SystemPrompt {
    fn from(value: String) -> Self {
        SystemPrompt::Text(value)
    }
}

impl From<&str> for SystemPrompt {
    fn from(value: &str) -> Self {
        SystemPrompt::Text(value.to_string())
    }
}

impl From<Vec<ContentBlock>> for SystemPrompt {
    fn from(value: Vec<ContentBlock>) -> Self {
        SystemPrompt::Blocks(value)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_message_request_serialization() {
        let req = CreateMessageRequest::new(
            "claude-sonnet-4-20250514",
            vec![ApiMessage {
                role: Role::User,
                content: ApiContent::Text("Hello".to_string()),
            }],
        )
        .with_max_tokens(1024)
        .with_stream(true);

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"claude-sonnet-4-20250514\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"stream\":true"));
    }

    #[test]
    fn test_create_message_request_skips_none_fields() {
        let req = CreateMessageRequest::new("claude-sonnet-4-20250514", vec![]);
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("system"));
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("stream"));
        assert!(!json.contains("tools"));
    }

    #[test]
    fn test_create_message_request_with_tools() {
        let tool = ApiTool::new(
            "Bash",
            "Execute a bash command",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }),
        );

        let req =
            CreateMessageRequest::new("claude-sonnet-4-20250514", vec![]).with_tools(vec![tool]);

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"name\":\"Bash\""));
        assert!(json.contains("\"input_schema\""));
    }

    #[test]
    fn test_api_content_text_serialization() {
        let content = ApiContent::Text("hello".to_string());
        let json = serde_json::to_string(&content).unwrap();
        assert_eq!(json, "\"hello\"");
    }

    #[test]
    fn test_api_content_blocks_serialization() {
        let content = ApiContent::Blocks(vec![ContentBlock::text("hello")]);
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
    }

    #[test]
    fn test_api_message_from_core_message() {
        let core_msg = rust_claude_core::message::Message::user("Hello!");
        let api_msg = ApiMessage::from(&core_msg);

        assert_eq!(api_msg.role, Role::User);
        match api_msg.content {
            ApiContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
            }
            _ => panic!("Expected blocks"),
        }
    }

    #[test]
    fn test_create_message_response_deserialization() {
        let json = r#"{
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hello!"}
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        }"#;

        let resp: CreateMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "msg_123");
        assert_eq!(resp.role, Role::Assistant);
        assert_eq!(resp.stop_reason, Some(StopReason::EndTurn));
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.content.len(), 1);
    }

    #[test]
    fn test_create_message_response_with_tool_use() {
        let json = r#"{
            "id": "msg_456",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me check."},
                {"type": "tool_use", "id": "tool_1", "name": "Bash", "input": {"command": "ls"}}
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 50,
                "output_tokens": 30
            }
        }"#;

        let resp: CreateMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.stop_reason, Some(StopReason::ToolUse));
        assert_eq!(resp.content.len(), 2);

        let tool_use = &resp.content[1];
        match tool_use {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls");
            }
            _ => panic!("Expected tool_use block"),
        }
    }

    #[test]
    fn test_api_error_deserialization() {
        let json = r#"{
            "error": {
                "type": "invalid_request_error",
                "message": "max_tokens: must be greater than 0"
            }
        }"#;

        let err: ApiErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(err.error.error_type, "invalid_request_error");
    }

    #[test]
    fn test_full_request_format_matches_anthropic_api() {
        let req = CreateMessageRequest::new(
            "claude-sonnet-4-20250514",
            vec![ApiMessage {
                role: Role::User,
                content: ApiContent::Blocks(vec![ContentBlock::text("List files")]),
            }],
        )
        .with_max_tokens(1024)
        .with_system("You are a helpful assistant.");

        let json = serde_json::to_string_pretty(&req).unwrap();

        let expected = r#"{
  "model": "claude-sonnet-4-20250514",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "List files"
        }
      ]
    }
  ],
  "system": "You are a helpful assistant.",
  "max_tokens": 1024
}"#;
        assert_eq!(json, expected);
    }

    #[test]
    fn test_system_prompt_blocks_serialization() {
        let req = CreateMessageRequest::new("claude-sonnet-4-20250514", vec![]).with_system(vec![
            ContentBlock::text("You are a helpful assistant."),
            ContentBlock::thinking("Reason carefully before using tools."),
        ]);

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"system\":[{"));
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"type\":\"thinking\""));
    }
}
