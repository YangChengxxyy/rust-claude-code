use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub metadata: ToolResultMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolResultMetadata {
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub truncated: bool,
}

impl ToolResult {
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
            metadata: ToolResultMetadata::default(),
        }
    }

    pub fn error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
            metadata: ToolResultMetadata::default(),
        }
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.metadata.duration_ms = Some(ms);
        self
    }

    pub fn with_truncated(mut self) -> Self {
        self.metadata.truncated = true;
        self
    }

    pub fn to_content_block(&self) -> crate::message::ContentBlock {
        crate::message::ContentBlock::tool_result(&self.tool_use_id, &self.content, self.is_error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("tool_1", "output");
        assert!(!result.is_error);
        assert_eq!(result.tool_use_id, "tool_1");
        assert_eq!(result.content, "output");
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("tool_1", "command failed");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_result_builder() {
        let result = ToolResult::success("tool_1", "output")
            .with_duration(150)
            .with_truncated();
        assert_eq!(result.metadata.duration_ms, Some(150));
        assert!(result.metadata.truncated);
    }

    #[test]
    fn test_tool_result_to_content_block() {
        let result = ToolResult::success("tool_1", "output");
        let block = result.to_content_block();
        match block {
            crate::message::ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool_1");
                assert_eq!(content, "output");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_tool_result_serde() {
        let result = ToolResult::success("tool_1", "output").with_duration(100);
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool_use_id, "tool_1");
        assert_eq!(parsed.metadata.duration_ms, Some(100));
    }
}
