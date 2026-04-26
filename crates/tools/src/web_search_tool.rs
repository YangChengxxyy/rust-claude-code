use async_trait::async_trait;
use std::sync::Arc;

use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{Tool, ToolContext, ToolError};
use crate::web::search::{SearchBackend, SearchResult};

#[derive(Debug, Clone, serde::Deserialize)]
struct WebSearchInput {
    query: String,
    #[serde(default)]
    allowed_domains: Vec<String>,
    #[serde(default)]
    blocked_domains: Vec<String>,
}

pub struct WebSearchTool {
    backend: Arc<dyn SearchBackend>,
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
        Self {
            backend: self.backend.clone(),
        }
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self::configured(WebSearchConfig::from_env())
    }

    pub fn configured(config: WebSearchConfig) -> Self {
        if !config.provider.eq_ignore_ascii_case("brave") {
            return Self {
                backend: Arc::new(UnavailableSearchBackend {
                    reason: format!("unsupported web search provider '{}'", config.provider),
                }),
            };
        }

        match config.brave_api_key {
            Some(api_key) if !api_key.trim().is_empty() => Self {
                backend: Arc::new(BraveSearchBackend::new(
                    api_key,
                    config.brave_base_url.unwrap_or_else(BraveSearchBackend::default_base_url),
                )),
            },
            _ => Self {
                backend: Arc::new(UnavailableSearchBackend {
                    reason: "missing Brave Search credentials; set BRAVE_SEARCH_API_KEY or RUST_CLAUDE_BRAVE_SEARCH_API_KEY".to_string(),
                }),
            },
        }
    }

    pub fn with_backend(backend: Arc<dyn SearchBackend>) -> Self {
        Self { backend }
    }

    /// Check if a URL's host matches the allowed/blocked domain lists.
    ///
    /// Matching requires the host to be either an exact match or a subdomain
    /// (e.g. `sub.example.com` matches `example.com`, but `evil-example.com`
    /// does not).
    fn domain_allowed(url: &str, allowed: &[String], blocked: &[String]) -> bool {
        let host = url
            .split("//")
            .nth(1)
            .unwrap_or(url)
            .split('/')
            .next()
            .unwrap_or(url);

        if blocked
            .iter()
            .any(|domain| Self::domain_matches(host, domain))
        {
            return false;
        }
        if allowed.is_empty() {
            return true;
        }
        allowed
            .iter()
            .any(|domain| Self::domain_matches(host, domain))
    }

    /// Return true if `host` is exactly `domain` or is a subdomain of `domain`.
    fn domain_matches(host: &str, domain: &str) -> bool {
        host == domain || host.ends_with(&format!(".{}", domain))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchConfig {
    pub provider: String,
    pub brave_api_key: Option<String>,
    pub brave_base_url: Option<String>,
}

impl WebSearchConfig {
    pub fn from_env() -> Self {
        let provider = std::env::var("RUST_CLAUDE_WEB_SEARCH_PROVIDER")
            .or_else(|_| std::env::var("WEB_SEARCH_PROVIDER"))
            .unwrap_or_else(|_| "brave".to_string());
        let brave_api_key = std::env::var("RUST_CLAUDE_BRAVE_SEARCH_API_KEY")
            .or_else(|_| std::env::var("BRAVE_SEARCH_API_KEY"))
            .ok();
        let brave_base_url = std::env::var("RUST_CLAUDE_BRAVE_SEARCH_BASE_URL")
            .or_else(|_| std::env::var("BRAVE_SEARCH_BASE_URL"))
            .ok();

        Self {
            provider,
            brave_api_key,
            brave_base_url,
        }
    }
}

#[derive(Debug)]
struct UnavailableSearchBackend {
    reason: String,
}

#[async_trait]
impl SearchBackend for UnavailableSearchBackend {
    fn name(&self) -> &str {
        "unavailable"
    }

    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>, String> {
        Err(self.reason.clone())
    }
}

#[derive(Debug, Clone)]
pub struct BraveSearchBackend {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl BraveSearchBackend {
    fn new(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: reqwest::Client::new(),
        }
    }

    fn default_base_url() -> String {
        "https://api.search.brave.com/res/v1/web/search".to_string()
    }

    fn parse_response(value: serde_json::Value) -> Result<Vec<SearchResult>, String> {
        let results = value
            .get("web")
            .and_then(|web| web.get("results"))
            .and_then(|results| results.as_array())
            .ok_or_else(|| "Brave Search response missing web.results".to_string())?;

        results
            .iter()
            .map(|item| {
                let title = item
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                let url = item
                    .get("url")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                let summary = item
                    .get("description")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();

                if title.is_empty() || url.is_empty() {
                    return Err("Brave Search result missing title or url".to_string());
                }

                Ok(SearchResult {
                    title,
                    url,
                    summary,
                })
            })
            .collect()
    }
}

#[async_trait]
impl SearchBackend for BraveSearchBackend {
    fn name(&self) -> &str {
        "brave"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let response = self
            .client
            .get(&self.base_url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &self.api_key)
            .query(&[("q", query), ("count", "10")])
            .send()
            .await
            .map_err(|error| format!("Brave Search request failed: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Brave Search returned HTTP status {}",
                response.status()
            ));
        }

        let value = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| format!("Brave Search response was not valid JSON: {error}"))?;
        Self::parse_response(value)
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "WebSearch".to_string(),
            description:
                "Run a web search and return formatted results with optional domain filters"
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

        let results =
            self.backend.search(&input.query).await.map_err(|error| {
                ToolError::Execution(format!("search backend failed: {}", error))
            })?;

        let filtered = results
            .into_iter()
            .filter(|result| {
                Self::domain_allowed(&result.url, &input.allowed_domains, &input.blocked_domains)
            })
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
    use crate::web::search::SearchBackend;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[derive(Debug)]
    struct FakeSearchBackend {
        results: Result<Vec<SearchResult>, String>,
    }

    #[async_trait]
    impl SearchBackend for FakeSearchBackend {
        fn name(&self) -> &str {
            "fake"
        }

        async fn search(&self, _query: &str) -> Result<Vec<SearchResult>, String> {
            self.results.clone()
        }
    }

    fn fake_tool(results: Result<Vec<SearchResult>, String>) -> WebSearchTool {
        WebSearchTool::with_backend(Arc::new(FakeSearchBackend { results }))
    }

    #[test]
    fn schema_requires_query() {
        let schema = WebSearchTool::new().info().input_schema;
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
    }

    #[test]
    fn config_reads_provider_and_credentials_from_env() {
        let _guard = env_lock().lock().unwrap();
        let old_provider = std::env::var("RUST_CLAUDE_WEB_SEARCH_PROVIDER").ok();
        let old_key = std::env::var("RUST_CLAUDE_BRAVE_SEARCH_API_KEY").ok();
        let old_base = std::env::var("RUST_CLAUDE_BRAVE_SEARCH_BASE_URL").ok();
        unsafe {
            std::env::set_var("RUST_CLAUDE_WEB_SEARCH_PROVIDER", "brave");
            std::env::set_var("RUST_CLAUDE_BRAVE_SEARCH_API_KEY", "test-key");
            std::env::set_var(
                "RUST_CLAUDE_BRAVE_SEARCH_BASE_URL",
                "http://localhost/search",
            );
        }

        let config = WebSearchConfig::from_env();

        match old_provider {
            Some(value) => unsafe { std::env::set_var("RUST_CLAUDE_WEB_SEARCH_PROVIDER", value) },
            None => unsafe { std::env::remove_var("RUST_CLAUDE_WEB_SEARCH_PROVIDER") },
        }
        match old_key {
            Some(value) => unsafe { std::env::set_var("RUST_CLAUDE_BRAVE_SEARCH_API_KEY", value) },
            None => unsafe { std::env::remove_var("RUST_CLAUDE_BRAVE_SEARCH_API_KEY") },
        }
        match old_base {
            Some(value) => unsafe { std::env::set_var("RUST_CLAUDE_BRAVE_SEARCH_BASE_URL", value) },
            None => unsafe { std::env::remove_var("RUST_CLAUDE_BRAVE_SEARCH_BASE_URL") },
        }

        assert_eq!(config.provider, "brave");
        assert_eq!(config.brave_api_key.as_deref(), Some("test-key"));
        assert_eq!(
            config.brave_base_url.as_deref(),
            Some("http://localhost/search")
        );
    }

    #[tokio::test]
    async fn success_formats_results() {
        let tool = fake_tool(Ok(vec![SearchResult {
            title: "Rust".into(),
            url: "https://www.rust-lang.org/".into(),
            summary: "Rust language".into(),
        }]));

        let result = tool
            .execute(
                serde_json::json!({ "query": "rust" }),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    ..ToolContext::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("Rust"));
        assert!(result.content.contains("https://www.rust-lang.org/"));
    }

    #[tokio::test]
    async fn empty_results_returns_no_results_message() {
        let result = fake_tool(Ok(vec![]))
            .execute(
                serde_json::json!({ "query": "rust" }),
                ToolContext {
                    tool_use_id: "tool_1".into(),
                    ..ToolContext::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "No search results");
    }

    #[tokio::test]
    async fn provider_errors_are_tool_errors() {
        let error = fake_tool(Err("provider exploded".into()))
            .execute(
                serde_json::json!({ "query": "rust" }),
                ToolContext::default(),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(error, ToolError::Execution(message) if message.contains("provider exploded"))
        );
    }

    #[test]
    fn brave_response_normalizes_results() {
        let results = BraveSearchBackend::parse_response(serde_json::json!({
            "web": {
                "results": [
                    {
                        "title": "Rust",
                        "url": "https://www.rust-lang.org/",
                        "description": "Rust language"
                    }
                ]
            }
        }))
        .unwrap();

        assert_eq!(
            results,
            vec![SearchResult {
                title: "Rust".into(),
                url: "https://www.rust-lang.org/".into(),
                summary: "Rust language".into(),
            }]
        );
    }

    #[test]
    fn malformed_brave_response_returns_error() {
        let error = BraveSearchBackend::parse_response(serde_json::json!({ "unexpected": true }))
            .unwrap_err();
        assert!(error.contains("web.results"));
    }

    #[tokio::test]
    async fn missing_credentials_return_clear_error() {
        let error = WebSearchTool::configured(WebSearchConfig {
            provider: "brave".into(),
            brave_api_key: None,
            brave_base_url: None,
        })
        .execute(
            serde_json::json!({ "query": "rust" }),
            ToolContext::default(),
        )
        .await
        .unwrap_err();

        assert!(
            matches!(error, ToolError::Execution(message) if message.contains("BRAVE_SEARCH_API_KEY"))
        );
    }

    #[tokio::test]
    async fn domain_filters_apply_to_backend_results() {
        let tool = fake_tool(Ok(vec![
            SearchResult {
                title: "Rust".into(),
                url: "https://www.rust-lang.org/".into(),
                summary: "Rust language".into(),
            },
            SearchResult {
                title: "Blocked".into(),
                url: "https://example.com/".into(),
                summary: "Example".into(),
            },
        ]));

        let result = tool
            .execute(
                serde_json::json!({
                    "query": "rust",
                    "allowed_domains": ["rust-lang.org"],
                    "blocked_domains": []
                }),
                ToolContext::default(),
            )
            .await
            .unwrap();

        assert!(result.content.contains("Rust"));
        assert!(!result.content.contains("Blocked"));
    }

    #[tokio::test]
    #[ignore = "requires BRAVE_SEARCH_API_KEY and live network"]
    async fn live_brave_search_returns_results_when_configured() {
        let api_key = std::env::var("BRAVE_SEARCH_API_KEY")
            .or_else(|_| std::env::var("RUST_CLAUDE_BRAVE_SEARCH_API_KEY"))
            .expect("set BRAVE_SEARCH_API_KEY");
        let backend = BraveSearchBackend::new(api_key, BraveSearchBackend::default_base_url());
        let results = backend.search("rust programming language").await.unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn domain_filter_logic() {
        // Exact match allowed
        assert!(WebSearchTool::domain_allowed(
            "https://docs.rs/foo",
            &vec!["docs.rs".into()],
            &[]
        ));
        // Subdomain match allowed
        assert!(WebSearchTool::domain_allowed(
            "https://sub.docs.rs/foo",
            &vec!["docs.rs".into()],
            &[]
        ));
        // Not in allowed list
        assert!(!WebSearchTool::domain_allowed(
            "https://example.com/foo",
            &vec!["docs.rs".into()],
            &[]
        ));
        // Blocked domain
        assert!(!WebSearchTool::domain_allowed(
            "https://example.com/foo",
            &[],
            &vec!["example.com".into()]
        ));
        // Subdomain spoofing must NOT match (evil-example.com != example.com)
        assert!(WebSearchTool::domain_allowed(
            "https://evil-example.com/foo",
            &[],
            &vec!["example.com".into()]
        ));
        // But a real subdomain IS blocked
        assert!(!WebSearchTool::domain_allowed(
            "https://sub.example.com/foo",
            &[],
            &vec!["example.com".into()]
        ));
    }
}
