use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "tool_use_id")]
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text { text: text.into() }
    }

    pub fn tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error,
        }
    }

    pub fn thinking(thinking: impl Into<String>) -> Self {
        ContentBlock::Thinking {
            thinking: thinking.into(),
        }
    }

    pub fn is_tool_use(&self) -> bool {
        matches!(self, ContentBlock::ToolUse { .. })
    }

    pub fn as_tool_use(&self) -> Option<(&str, &str, &serde_json::Value)> {
        match self {
            ContentBlock::ToolUse { id, name, input } => Some((id, name, input)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::Assistant,
            content,
        }
    }

    pub fn user_with_blocks(content: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::User,
            content,
        }
    }

    pub fn tool_results(tool_uses: &[(String, String, bool)]) -> Self {
        let content: Vec<ContentBlock> = tool_uses
            .iter()
            .map(|(id, result, is_error)| {
                ContentBlock::tool_result(id.clone(), result.clone(), *is_error)
            })
            .collect();
        Message {
            role: Role::User,
            content,
        }
    }

    pub fn has_tool_use(&self) -> bool {
        self.content.iter().any(|b| b.is_tool_use())
    }

    pub fn tool_uses(&self) -> Vec<(&str, &str, &serde_json::Value)> {
        self.content
            .iter()
            .filter_map(|b| b.as_tool_use())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_role_serde() {
        let json = serde_json::to_string(&Role::User).unwrap();
        assert_eq!(json, "\"user\"");

        let json = serde_json::to_string(&Role::Assistant).unwrap();
        assert_eq!(json, "\"assistant\"");

        let role: Role = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(role, Role::User);
    }

    #[test]
    fn test_stop_reason_serde() {
        let json = serde_json::to_string(&StopReason::ToolUse).unwrap();
        assert_eq!(json, "\"tool_use\"");

        let sr: StopReason = serde_json::from_str("\"end_turn\"").unwrap();
        assert_eq!(sr, StopReason::EndTurn);
    }

    #[test]
    fn test_content_block_text_serde() {
        let block = ContentBlock::text("hello world");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"hello world\""));

        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, block);
    }

    #[test]
    fn test_content_block_tool_use_serde() {
        let block = ContentBlock::tool_use("id_123", "Bash", serde_json::json!({"command": "ls"}));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"tool_use\""));
        assert!(json.contains("\"id\":\"id_123\""));
        assert!(json.contains("\"name\":\"Bash\""));

        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, block);
    }

    #[test]
    fn test_content_block_tool_result_serde() {
        let block = ContentBlock::tool_result("id_123", "output here", false);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"tool_result\""));
        assert!(json.contains("\"tool_use_id\":\"id_123\""));

        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, block);
    }

    #[test]
    fn test_content_block_thinking_serde() {
        let block = ContentBlock::thinking("internal reasoning");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"thinking\""));

        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, block);
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello, Claude!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant(vec![
            ContentBlock::text("I'll help you with that."),
            ContentBlock::tool_use("tool_1", "Bash", serde_json::json!({"command": "pwd"})),
        ]);
        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.has_tool_use());
        assert_eq!(msg.tool_uses().len(), 1);
    }

    #[test]
    fn test_message_serializes_to_anthropic_format() {
        let msg = Message::user("What files are here?");
        let json = serde_json::to_string_pretty(&msg).unwrap();

        let expected = r#"{
  "role": "user",
  "content": [
    {
      "type": "text",
      "text": "What files are here?"
    }
  ]
}"#;
        assert_eq!(json, expected);
    }

    #[test]
    fn test_message_with_tool_results() {
        let msg = Message::tool_results(&[(
            "tool_1".to_string(),
            "file1.txt\nfile2.txt".to_string(),
            false,
        )]);
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"tool_result\""));
        assert!(json.contains("\"tool_use_id\":\"tool_1\""));
    }

    #[test]
    fn test_usage_serde() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: Usage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input_tokens, 100);
        assert_eq!(parsed.output_tokens, 50);
    }
}
