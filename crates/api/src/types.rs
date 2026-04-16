use serde::{Deserialize, Serialize};

use rust_claude_core::message::{ContentBlock, Role, StopReason, Usage};

/// Anthropic prompt caching control marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheControl {
    pub r#type: String,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            r#type: "ephemeral".to_string(),
        }
    }
}

/// A structured system prompt block with optional cache control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemBlock {
    pub r#type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SystemBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            r#type: "text".to_string(),
            text: text.into(),
            cache_control: None,
        }
    }

    pub fn with_cache_control(mut self) -> Self {
        self.cache_control = Some(CacheControl::ephemeral());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestMetadata>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: Role,
    pub content: ApiContent,
}

impl ApiMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: ApiContent::Text(text.into()),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: ApiContent::Text(text.into()),
        }
    }
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
    StructuredBlocks(Vec<SystemBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn input_schema(&self) -> &serde_json::Value {
        &self.input_schema
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
            metadata: None,
            max_tokens: None,
            stream: None,
            tools: None,
            stop_sequences: None,
            temperature: None,
            thinking: None,
        }
    }

    pub fn with_system(mut self, system: impl Into<SystemPrompt>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_system_opt(mut self, system: Option<String>) -> Self {
        self.system = system.map(SystemPrompt::Text);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = Some(metadata);
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

    pub fn with_thinking(mut self, thinking: serde_json::Value) -> Self {
        self.thinking = Some(thinking);
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

impl From<Vec<SystemBlock>> for SystemPrompt {
    fn from(value: Vec<SystemBlock>) -> Self {
        SystemPrompt::StructuredBlocks(value)
    }
}

/// Inject `cache_control: { type: "ephemeral" }` on the last content block
/// of the last message in a serialized messages array.
///
/// This modifies the JSON value in-place without changing the core types.
pub fn inject_cache_control_on_messages(messages: &mut Vec<serde_json::Value>) {
    if let Some(last_msg) = messages.last_mut() {
        if let Some(content) = last_msg.get_mut("content") {
            match content {
                serde_json::Value::Array(blocks) => {
                    if let Some(last_block) = blocks.last_mut() {
                        if let serde_json::Value::Object(map) = last_block {
                            map.insert(
                                "cache_control".to_string(),
                                serde_json::json!({"type": "ephemeral"}),
                            );
                        }
                    }
                }
                serde_json::Value::String(_) => {
                    // Single string content — wrap into block form to add cache_control
                    // This shouldn't happen in practice as we always use blocks, but handle gracefully
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicErrorBody {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorResponse {
    pub error: AnthropicErrorBody,
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
    fn test_create_message_request_with_optional_system_prompt() {
        let req = CreateMessageRequest::new("claude-sonnet-4-20250514", vec![])
            .with_system_opt(Some("You are concise".to_string()));

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"system\":\"You are concise\""));

        let req =
            CreateMessageRequest::new("claude-sonnet-4-20250514", vec![]).with_system_opt(None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"system\""));
    }

    #[test]
    fn test_create_message_request_skips_none_fields() {
        let req = CreateMessageRequest::new("claude-sonnet-4-20250514", vec![]);
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("system"));
        assert!(!json.contains("metadata"));
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("stream"));
        assert!(!json.contains("tools"));
    }

    #[test]
    fn test_create_message_request_with_metadata() {
        let req = CreateMessageRequest::new("claude-sonnet-4-20250514", vec![]).with_metadata(
            RequestMetadata {
                user_id: Some("user-123".to_string()),
            },
        );

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"metadata\":{\"user_id\":\"user-123\"}"));
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

    // -- CacheControl & SystemBlock tests --

    #[test]
    fn test_cache_control_serialization() {
        let cc = CacheControl::ephemeral();
        let json = serde_json::to_string(&cc).unwrap();
        assert_eq!(json, r#"{"type":"ephemeral"}"#);
    }

    #[test]
    fn test_system_block_without_cache_control() {
        let block = SystemBlock::text("You are helpful");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"text\":\"You are helpful\""));
        assert!(!json.contains("cache_control"));
    }

    #[test]
    fn test_system_block_with_cache_control() {
        let block = SystemBlock::text("You are helpful").with_cache_control();
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"cache_control\":{\"type\":\"ephemeral\"}"));
    }

    #[test]
    fn test_structured_blocks_system_prompt() {
        let blocks = vec![
            SystemBlock::text("Section 1"),
            SystemBlock::text("Section 2").with_cache_control(),
        ];
        let prompt = SystemPrompt::StructuredBlocks(blocks);
        let json = serde_json::to_string(&prompt).unwrap();
        // Should be an array of objects
        assert!(json.starts_with('['));
        // Only the last block should have cache_control
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!(arr[0].get("cache_control").is_none());
        assert!(arr[1].get("cache_control").is_some());
    }

    // -- Thinking field tests --

    #[test]
    fn test_request_with_thinking() {
        let req = CreateMessageRequest::new("claude-opus-4-6", vec![])
            .with_thinking(serde_json::json!({"type": "enabled", "budget_tokens": 10000}));
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"thinking\""));
        assert!(json.contains("\"budget_tokens\":10000"));
    }

    #[test]
    fn test_request_without_thinking_omits_field() {
        let req = CreateMessageRequest::new("claude-opus-4-6", vec![]);
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"thinking\""));
    }

    // -- inject_cache_control_on_messages tests --

    #[test]
    fn test_inject_cache_control_single_message() {
        let msg = ApiMessage {
            role: Role::User,
            content: ApiContent::Blocks(vec![ContentBlock::text("hello")]),
        };
        let mut messages: Vec<serde_json::Value> = vec![serde_json::to_value(&msg).unwrap()];
        inject_cache_control_on_messages(&mut messages);

        let content = messages[0]["content"].as_array().unwrap();
        assert!(content[0].get("cache_control").is_some());
    }

    #[test]
    fn test_inject_cache_control_multiple_messages() {
        let msg1 = serde_json::to_value(&ApiMessage {
            role: Role::User,
            content: ApiContent::Blocks(vec![ContentBlock::text("first")]),
        })
        .unwrap();
        let msg2 = serde_json::to_value(&ApiMessage {
            role: Role::Assistant,
            content: ApiContent::Blocks(vec![ContentBlock::text("second")]),
        })
        .unwrap();
        let mut messages = vec![msg1, msg2];
        inject_cache_control_on_messages(&mut messages);

        // Only last message should have cache_control
        let content0 = messages[0]["content"].as_array().unwrap();
        assert!(content0[0].get("cache_control").is_none());
        let content1 = messages[1]["content"].as_array().unwrap();
        assert!(content1[0].get("cache_control").is_some());
    }

    #[test]
    fn test_inject_cache_control_multi_block_message() {
        let msg = serde_json::to_value(&ApiMessage {
            role: Role::User,
            content: ApiContent::Blocks(vec![
                ContentBlock::tool_result("t1", "result1", false),
                ContentBlock::tool_result("t2", "result2", false),
            ]),
        })
        .unwrap();
        let mut messages = vec![msg];
        inject_cache_control_on_messages(&mut messages);

        let content = messages[0]["content"].as_array().unwrap();
        // Only the last block should have cache_control
        assert!(content[0].get("cache_control").is_none());
        assert!(content[1].get("cache_control").is_some());
    }

    #[test]
    fn test_inject_cache_control_empty_messages() {
        let mut messages: Vec<serde_json::Value> = vec![];
        inject_cache_control_on_messages(&mut messages);
        assert!(messages.is_empty());
    }
}
