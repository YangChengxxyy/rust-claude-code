use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub summary: String,
}

#[async_trait]
pub trait SearchBackend: Send + Sync {
    fn name(&self) -> &str;
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String>;
}

#[derive(Debug, Default)]
pub struct DummySearchBackend;

#[async_trait]
impl SearchBackend for DummySearchBackend {
    fn name(&self) -> &str {
        "dummy"
    }

    async fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        Ok(vec![SearchResult {
            title: format!("Result for {query}"),
            url: "https://example.com/result".to_string(),
            summary: "Dummy search result used for initial implementation/testing".to_string(),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_backend_has_name() {
        assert_eq!(DummySearchBackend.name(), "dummy");
    }

    #[tokio::test]
    async fn dummy_backend_returns_result() {
        let results = DummySearchBackend.search("rust").await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("rust"));
    }
}
