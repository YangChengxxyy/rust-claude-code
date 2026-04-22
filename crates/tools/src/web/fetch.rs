use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;

// Pre-compiled regexes for html_to_text — compiled once, reused on every call.
static RE_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script.*?</script>").unwrap());
static RE_STYLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style.*?</style>").unwrap());
static RE_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<[^>]+>").unwrap());
static RE_WHITESPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t\x0B\f\r]+").unwrap());

// Shared HTTP client — reuses connection pool across fetches.
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("rust-claude/0.1")
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Clone)]
pub struct WebPage {
    pub url: String,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct WebCache {
    entries: HashMap<String, (Instant, WebPage)>,
}

impl WebCache {
    pub fn get(&self, url: &str, ttl: Duration) -> Option<WebPage> {
        self.entries.get(url).and_then(|(created_at, page)| {
            if created_at.elapsed() <= ttl {
                Some(page.clone())
            } else {
                None
            }
        })
    }

    pub fn insert(&mut self, page: WebPage) {
        self.entries
            .insert(page.url.clone(), (Instant::now(), page));
    }
}

pub async fn fetch_url(url: &str) -> Result<String, reqwest::Error> {
    let response = HTTP_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    response.text().await
}

pub fn html_to_text(html: &str) -> String {
    // Strip scripts, styles, tags, then collapse whitespace.
    let without_scripts = RE_SCRIPT.replace_all(html, " ");
    let without_styles = RE_STYLE.replace_all(&without_scripts, " ");
    let without_tags = RE_TAG.replace_all(&without_styles, " ");
    let decoded = without_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'");
    RE_WHITESPACE
        .replace_all(&decoded, " ")
        .to_string()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn apply_prompt(content: &str, prompt: Option<&str>) -> String {
    match prompt.filter(|p| !p.trim().is_empty()) {
        Some(prompt) => format!("Prompt: {}\n\n{}", prompt.trim(), content),
        None => content.to_string(),
    }
}

pub fn truncate_text(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let truncated = content.chars().take(max_chars).collect::<String>();
    format!("{}\n\n[truncated]", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html() {
        let html = "<html><body><h1>Title</h1><p>Hello <b>world</b></p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn truncates_text() {
        let text = truncate_text("abcdef", 3);
        assert!(text.contains("abc"));
        assert!(text.contains("[truncated]"));
    }

    #[test]
    fn cache_hit_within_ttl() {
        let mut cache = WebCache::default();
        cache.insert(WebPage {
            url: "https://example.com".into(),
            content: "hello".into(),
        });
        let hit = cache.get("https://example.com", Duration::from_secs(60));
        assert!(hit.is_some());
    }
}
