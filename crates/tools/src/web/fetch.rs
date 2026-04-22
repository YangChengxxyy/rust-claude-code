use std::collections::HashMap;
use std::time::{Duration, Instant};

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
    let response = reqwest::Client::new().get(url).send().await?;
    response.text().await
}

pub fn html_to_text(html: &str) -> String {
    // Minimal first pass: strip scripts/styles/tags and collapse whitespace.
    let without_scripts = regex::Regex::new(r"(?is)<script.*?</script>")
        .unwrap()
        .replace_all(html, " ");
    let without_styles = regex::Regex::new(r"(?is)<style.*?</style>")
        .unwrap()
        .replace_all(&without_scripts, " ");
    let without_tags = regex::Regex::new(r"(?is)<[^>]+>")
        .unwrap()
        .replace_all(&without_styles, " ");
    let decoded = without_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    regex::Regex::new(r"[ \t\x0B\f\r]+")
        .unwrap()
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
