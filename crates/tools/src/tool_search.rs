use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use std::sync::Weak;

use crate::registry::ToolRegistry;
use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, serde::Deserialize)]
struct ToolSearchInput {
    query: String,
    #[serde(default)]
    max_results: Option<usize>,
}

pub struct ToolSearchTool {
    registry: Weak<ToolRegistry>,
}

impl ToolSearchTool {
    pub fn new(registry: Weak<ToolRegistry>) -> Self {
        Self { registry }
    }

    fn clamp_max_results(max: Option<usize>) -> usize {
        let val = max.unwrap_or(5);
        if val == 0 {
            5
        } else if val > 20 {
            20
        } else {
            val
        }
    }
}

#[async_trait]
impl Tool for ToolSearchTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "ToolSearch".to_string(),
            description:
                "Search for deferred tools by name or keyword to discover their full schema"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query. Use plain keywords or 'select:ToolName' for exact selection"
                    },
                    "max_results": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 20,
                        "description": "Maximum number of results to return (default: 5)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: ToolSearchInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let max_results = Self::clamp_max_results(input.max_results);

        let registry = self.registry.upgrade().ok_or_else(|| {
            ToolError::Execution("tool registry is no longer available".to_string())
        })?;

        let results = registry.search_tools(&input.query, max_results);

        let output: Vec<serde_json::Value> = results
            .into_iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.info.name,
                    "description": tool.info.description,
                    "input_schema": tool.info.input_schema,
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&output)
            .map_err(|error| ToolError::Execution(error.to_string()))?;

        Ok(ToolResult::success(context.tool_use_id, json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bash::BashTool;
    use crate::web_search_tool::WebSearchTool;
    use std::sync::Arc;

    fn make_registry() -> Arc<ToolRegistry> {
        Arc::new_cyclic(|weak| {
            let registry = ToolRegistry::new();
            registry.register(BashTool::new());
            registry.register(WebSearchTool::new());
            registry.register(ToolSearchTool::new(weak.clone()));
            registry
        })
    }

    #[test]
    fn test_tool_search_tool_info() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        assert_eq!(tool.info.name, "ToolSearch");
    }

    #[test]
    fn test_tool_search_tool_is_read_only() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        assert!(tool.is_read_only);
    }

    #[test]
    fn test_tool_search_tool_should_not_defer() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        assert!(!tool.should_defer);
    }

    #[tokio::test]
    async fn test_execute_keyword_search() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "web" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        // Should find WebSearchTool
        assert!(result.content.contains("WebSearch"));
    }

    #[tokio::test]
    async fn test_execute_exact_selection() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "select:WebSearch" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("WebSearch"));
        // Bash is not deferred, should not appear
        assert!(!result.content.contains("Bash"));
    }

    #[tokio::test]
    async fn test_execute_respects_max_results() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "web", "max_results": 1 }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        // Should still find WebSearchTool with max 1
        assert!(result.content.contains("WebSearch"));
    }

    #[tokio::test]
    async fn test_execute_clamps_max_results_above_20() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "web", "max_results": 100 }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_execute_empty_results() {
        let registry = make_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "nonexistent_xyz" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.content, "[]");
    }

    #[test]
    fn test_clamp_max_results() {
        assert_eq!(ToolSearchTool::clamp_max_results(None), 5);
        assert_eq!(ToolSearchTool::clamp_max_results(Some(3)), 3);
        assert_eq!(ToolSearchTool::clamp_max_results(Some(25)), 20);
        assert_eq!(ToolSearchTool::clamp_max_results(Some(0)), 5);
    }
}
