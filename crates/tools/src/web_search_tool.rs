use async_trait::async_trait;

use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{Tool, ToolContext, ToolError};
use crate::web::search::{DummySearchBackend, SearchBackend};

#[derive(Debug, Clone, serde::Deserialize)]
struct WebSearchInput {
    query: String,
    #[serde(default)]
    allowed_domains: Vec<String>,
    #[serde(default)]
    blocked_domains: Vec<String>,
}

pub struct WebSearchTool {
    backend: Box<dyn SearchBackend>,
}

impl std::fmt::Debug for WebSearchTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSearchTool")
            .field("backend", &self.backend.name())
            .finish()
    }
}

impl Clone for WebSearchTool {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            backend: Box::new(DummySearchBackend),
        }
    }

    fn domain_allowed(url: &str, allowed: &[String], blocked: &[String]) -> bool {
        let host = url
            .split("//")
            .nth(1)
            .unwrap_or(url)
            .split('/')
            .next()
            .unwrap_or(url);

        if blocked.iter().any(|domain| host.ends_with(domain)) {
            return false;
        }
        if allowed.is_empty() {
            return true;
        }
        allowed.iter().any(|domain| host.ends_with(domain))
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "WebSearch".to_string(),
            description: "Run a web search and return formatted results with optional domain filters"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "allowed_domains": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "blocked_domains": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: WebSearchInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let results = self
            .backend
            .search(&input.query)
            .map_err(|error| ToolError::Execution(format!("search backend failed: {}", error)))?;

        let filtered = results
            .into_iter()
            .filter(|result| Self::domain_allowed(&result.url, &input.allowed_domains, &input.blocked_domains))
            .collect::<Vec<_>>();

        let content = if filtered.is_empty() {
            "No search results".to_string()
        } else {
            filtered
                .iter()
                .map(|result| format!("- {}\n  {}\n  {}", result.title, result.url, result.summary))
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ToolResult::success(context.tool_use_id, content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_query() {
        let schema = WebSearchTool::new().info().input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
    }

    #[test]
    fn domain_filter_logic() {
        assert!(WebSearchTool::domain_allowed(
            "https://docs.rs/foo",
            &vec!["docs.rs".into()],
            &[]
        ));
        assert!(!WebSearchTool::domain_allowed(
            "https://example.com/foo",
            &vec!["docs.rs".into()],
            &[]
        ));
        assert!(!WebSearchTool::domain_allowed(
            "https://example.com/foo",
            &[],
            &vec!["example.com".into()]
        ));
    }
}
