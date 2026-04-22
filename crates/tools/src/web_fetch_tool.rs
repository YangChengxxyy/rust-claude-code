use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{Tool, ToolContext, ToolError};
use crate::web::{truncate_text, WebCache, WebPage};

const DEFAULT_TTL_SECS: u64 = 15 * 60;
const DEFAULT_MAX_CHARS: usize = 12_000;

#[derive(Debug, Clone, serde::Deserialize)]
struct WebFetchInput {
    url: String,
    #[serde(default)]
    prompt: Option<String>,
}

#[derive(Clone)]
pub struct WebFetchTool {
    cache: Arc<Mutex<WebCache>>,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(WebCache::default())),
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "WebFetch".to_string(),
            description: "Fetch a web page, extract readable content, and return a truncated text result"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "prompt": { "type": "string" }
                },
                "required": ["url"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: WebFetchInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        let ttl = Duration::from_secs(DEFAULT_TTL_SECS);
        let page = {
            let cache = self.cache.lock().await;
            cache.get(&input.url, ttl)
        };

        let page = if let Some(page) = page {
            page
        } else {
            let html = crate::web::fetch::fetch_url(&input.url)
                .await
                .map_err(|error| ToolError::Execution(format!("web fetch failed: {}", error)))?;
            let text = crate::web::fetch::html_to_text(&html);
            let content = crate::web::fetch::apply_prompt(&text, input.prompt.as_deref());
            let page = WebPage {
                url: input.url.clone(),
                content,
            };
            let mut cache = self.cache.lock().await;
            cache.insert(page.clone());
            page
        };

        let output = truncate_text(&page.content, DEFAULT_MAX_CHARS);
        Ok(ToolResult::success(context.tool_use_id, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_requires_url() {
        let schema = WebFetchTool::new().info().input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "url"));
    }
}
