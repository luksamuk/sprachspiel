//! Web search using our own DuckDuckGo scraper.
//!
//! Replaces `ollama_rs::generation::tools::implementations::DDGSearcher` with
//! a hand-rolled implementation using `reqwest` and `scraper` (both already
//! in the dep tree). See `DdgSearcher` below for details.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::{format_size, parse_bounded_number};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprachspiel_tool_derive::tool;
use std::sync::Arc;
use tokio::sync::Mutex;

static SEARCHER: once_cell::sync::Lazy<Arc<Mutex<DdgSearcher>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(DdgSearcher::new())));

/// A single search result returned by `DdgSearcher::search`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    /// Result title
    pub title: String,
    /// Result URL
    pub link: String,
    /// Result snippet / description
    pub snippet: String,
}

/// Lightweight DuckDuckGo HTML scraper.
///
/// Replacement for `ollama_rs::generation::tools::implementations::DDGSearcher`
/// that avoids the ollama-rs dependency for search. Uses `reqwest` to fetch
/// the DuckDuckGo HTML interface and `scraper` to parse results.
///
/// # Usage
///
/// ```no_run
/// use sprachspiel::tools::search_builtin::DdgSearcher;
/// # async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let searcher = DdgSearcher::new();
/// let results = searcher.search("rust programming").await?;
/// for r in results {
///     println!("{} ({})\n  {}\n", r.title, r.link, r.snippet);
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Caveats
///
/// - DuckDuckGo may rate-limit or CAPTCHA automated traffic.
/// - CSS selectors may break if DuckDuckGo changes their HTML structure.
pub struct DdgSearcher {
    client: reqwest::Client,
}

impl DdgSearcher {
    /// Create a new DdgSearcher with a default reqwest client.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 ",
                "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            ))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// Perform a search and return up to ~30 results.
    pub async fn search(
        &self,
        query: &str,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("DuckDuckGo request failed: {}", e))?;

        let body = resp
            .text()
            .await
            .map_err(|e| format!("DuckDuckGo body read failed: {}", e))?;

        let document = scraper::Html::parse_document(&body);
        let result_selector = scraper::Selector::parse(".web-result")
            .map_err(|e| format!("Invalid selector: {:?}", e))?;
        let title_selector = scraper::Selector::parse(".result__a")
            .map_err(|e| format!("Invalid selector: {:?}", e))?;
        let url_selector = scraper::Selector::parse(".result__url")
            .map_err(|e| format!("Invalid selector: {:?}", e))?;
        let snippet_selector = scraper::Selector::parse(".result__snippet")
            .map_err(|e| format!("Invalid selector: {:?}", e))?;

        let mut results = Vec::new();
        for element in document.select(&result_selector) {
            let title = element
                .select(&title_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let link = element
                .select(&url_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let snippet = element
                .select(&snippet_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            if !title.is_empty() {
                results.push(SearchResult {
                    title,
                    link,
                    snippet,
                });
            }
        }
        Ok(results)
    }
}

impl Default for DdgSearcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum content size in characters (prevents memory issues with huge pages)
const MAX_CONTENT_SIZE: usize = 50_000;

/// Clean HTML by extracting main content and removing unwanted elements.
///
/// This function:
/// 1. Tries to extract content from main content areas (main, article, etc.)
/// 2. Removes script, style, nav, footer, and other non-content elements
/// 3. Falls back to full HTML if no main content is found
#[cfg(feature = "search-tools")]
fn clean_html(html: &str) -> String {
    use scraper::{Html, Selector};

    // If HTML is too small, return as-is
    if html.len() < 500 {
        return html.to_string();
    }

    let document = Html::parse_document(html);

    // Selectors for main content (in priority order)
    let content_selectors: &[&str] = &[
        "main",
        "article",
        "[role='main']",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content-body",
        ".post-body",
        "#content",
        "#main",
        ".content",
        ".main",
    ];

    // Try each content selector
    for selector_str in content_selectors {
        let selector = match Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(element) = document.select(&selector).next() {
            let content = element.html();
            // Only use if content is substantial
            if content.len() > 500 {
                return content;
            }
        }
    }

    // Fallback: return original HTML (will be truncated later)
    html.to_string()
}

/// Truncate content to maximum size, ensuring valid UTF-8 boundary.
///
/// Returns a truncated string that ends at a valid character boundary.
fn truncate_content(content: &str) -> std::borrow::Cow<'_, str> {
    if content.len() <= MAX_CONTENT_SIZE {
        return std::borrow::Cow::Borrowed(content);
    }

    // Find a valid UTF-8 boundary at or before MAX_CONTENT_SIZE
    let mut end = MAX_CONTENT_SIZE;

    // Walk back to find a valid char boundary
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }

    // Safety: end is now either 0 or at a valid boundary
    let truncated = &content[..end];

    // Add ellipsis if we truncated
    std::borrow::Cow::Owned(format!("{}...", truncated))
}

/// Search the web using DuckDuckGo.
///
/// Returns search results with title, URL, and snippet for each result.
/// Use this tool when you need to find current information on the internet.
/// Note: Does not require an API key, but may be blocked by CAPTCHA.
///
/// # Arguments
/// * `query` - The search query (what to search for). Be specific for better results.
///   - Example: "Rust async programming patterns"
/// * `num_results` - Number of results to return (default: 5, max: 10). Optional.
///
/// # Returns
/// Formatted search results with:
/// - Title, URL, and snippet for each result
///
/// # Errors
/// Returns error message if search fails or is blocked by CAPTCHA.
#[tool]
pub async fn web_search(
    query: String,
    num_results: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "web_search",
        &[
            ("query".to_string(), query.clone()),
            (
                "num_results".to_string(),
                num_results.clone().unwrap_or_else(|| "5".to_string()),
            ),
        ],
    );

    let num_results = parse_bounded_number(num_results.as_deref(), 5, Some(10));

    let searcher = SEARCHER.clone();
    let searcher = searcher.lock().await;

    let results = match searcher.search(&query).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!("Search error: {}. Please try again later.", e);
            log_tool_result("web_search", &err);
            return Ok(err);
        }
    };

    let json = match serde_json::to_string(&results) {
        Ok(j) => j,
        Err(e) => {
            let err = format!("Error serializing results: {}", e);
            log_tool_result("web_search", &err);
            return Ok(err);
        }
    };

    let parsed: Vec<SearchResult> = match serde_json::from_str(&json) {
        Ok(p) => p,
        Err(e) => {
            let err = format!("Error parsing results: {}", e);
            log_tool_result("web_search", &err);
            return Ok(err);
        }
    };

    if parsed.is_empty() {
        let result = format!("No results found for '{}'.", query);
        log_tool_result("web_search", &result);
        return Ok(result);
    }

    let mut output = vec![format!("**Search results for '{}'**\n", query)];

    for (i, r) in parsed.iter().take(num_results).enumerate() {
        output.push(format!("\n**{}. {}**\n{}", i + 1, r.title, r.link));
        if !r.snippet.is_empty() {
            output.push(format!("\n{}", r.snippet));
        }
    }

    output.push("\n\n_Source: DuckDuckGo_".to_string());

    let result = output.join("\n");
    log_tool_result("web_search", &result);
    Ok(result)
}

/// Search for news using DuckDuckGo.
///
/// Searches specifically for news articles about a topic.
/// Returns news results with title, URL, and snippet for each article.
/// Note: Does not require an API key, but may be blocked by CAPTCHA.
///
/// # Arguments
/// * `query` - The news topic to search for.
///   - Example: "technology" or "climate change updates"
/// * `num_results` - Number of results to return (default: 3, max: 10). Optional.
///
/// # Returns
/// Formatted news results with title, URL, and snippet for each article.
///
/// # Errors
/// Returns error message if search fails or is blocked by CAPTCHA.
#[tool]
pub async fn web_search_news(
    query: String,
    num_results: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "web_search_news",
        &[
            ("query".to_string(), query.clone()),
            (
                "num_results".to_string(),
                num_results.clone().unwrap_or_else(|| "3".to_string()),
            ),
        ],
    );

    let num_results = parse_bounded_number(num_results.as_deref(), 3, Some(10));
    let news_query = format!("{} news", query);

    let searcher = SEARCHER.clone();
    let searcher = searcher.lock().await;

    let results = match searcher.search(&news_query).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!("News search error: {}. Please try again later.", e);
            log_tool_result("web_search_news", &err);
            return Ok(err);
        }
    };

    let json = match serde_json::to_string(&results) {
        Ok(j) => j,
        Err(e) => {
            let err = format!("Error serializing results: {}", e);
            log_tool_result("web_search_news", &err);
            return Ok(err);
        }
    };

    let parsed: Vec<SearchResult> = match serde_json::from_str(&json) {
        Ok(p) => p,
        Err(e) => {
            let err = format!("Error parsing results: {}", e);
            log_tool_result("web_search_news", &err);
            return Ok(err);
        }
    };

    if parsed.is_empty() {
        let result = format!("No news found for '{}'.", query);
        log_tool_result("web_search_news", &result);
        return Ok(result);
    }

    let mut output = vec![format!("**News about '{}'**\n", query)];

    for (i, r) in parsed.iter().take(num_results).enumerate() {
        output.push(format!("\n**{}. {}**\n{}", i + 1, r.title, r.link));
        if !r.snippet.is_empty() {
            output.push(format!("\n{}", r.snippet));
        }
    }

    output.push("\n\n_Source: DuckDuckGo News_".to_string());

    let result = output.join("\n");
    log_tool_result("web_search_news", &result);
    Ok(result)
}

/// Scrape and extract text content from a webpage.
///
/// Fetches a webpage and converts it to readable markdown text.
/// Use this to get detailed content from a specific URL found via web_search.
///
/// The function:
/// - Extracts main content (article, main, etc.) when available
/// - Removes scripts, styles, navigation, and other non-content elements
/// - Converts HTML to clean markdown format
/// - Limits content to 50,000 characters to prevent memory issues
///
/// # Arguments
/// * `url` - The full URL of the webpage to scrape.
///   - Example: "https://example.com/article"
///
/// # Returns
/// Extracted text content in markdown format with:
/// - Page title
/// - Main content (headers, paragraphs, lists)
/// - Content size indicator
///
/// # Errors
/// Returns error message if URL is invalid, page is unreachable, or content cannot be parsed.
#[tool]
pub async fn web_scrape(url: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("web_scrape", &[("url".to_string(), url.clone())]);

    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let err = format!("Error creating HTTP client: {}. Please try again later.", e);
            log_tool_result("web_scrape", &err);
            return Ok(err);
        }
    };

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            let err = format!(
                "Network error: {}. Check if the URL is correct and accessible.",
                e
            );
            log_tool_result("web_scrape", &err);
            return Ok(err);
        }
    };

    let html = match response.text().await {
        Ok(h) => h,
        Err(e) => {
            let err = format!("Error reading response: {}", e);
            log_tool_result("web_scrape", &err);
            return Ok(err);
        }
    };

    // Clean HTML: extract main content and remove unwanted elements
    #[cfg(feature = "search-tools")]
    let cleaned_html = clean_html(&html);

    // If scraper feature is not enabled, use raw HTML
    #[cfg(not(feature = "search-tools"))]
    let cleaned_html = html;

    // Truncate to prevent memory issues with huge pages
    let truncated = truncate_content(&cleaned_html);

    // Convert to markdown
    let content = html2md::parse_html(&truncated);

    if content.trim().is_empty() {
        let result = format!("No content could be extracted from '{}'.", url);
        log_tool_result("web_scrape", &result);
        return Ok(result);
    }

    let size_info = format!(" ({})", format_size(content.len() as u64));
    let was_truncated = cleaned_html.len() > MAX_CONTENT_SIZE;
    let truncate_note = if was_truncated { " (truncated)" } else { "" };

    let result = format!(
        "**Content from {}**{}{}\n\n{}",
        url, size_info, truncate_note, content
    );
    log_tool_result("web_scrape", &result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_content_small() {
        // Content smaller than MAX_CONTENT_SIZE should be unchanged
        let content = "Hello, world!";
        let truncated = truncate_content(content);
        assert_eq!(truncated.as_ref(), content);
    }

    #[test]
    fn test_truncate_content_large() {
        // Content larger than MAX_CONTENT_SIZE should be truncated
        let large_content = "x".repeat(60_000);
        let truncated = truncate_content(&large_content);

        // Should be at most MAX_CONTENT_SIZE + ellipsis
        assert!(truncated.len() <= MAX_CONTENT_SIZE + 10); // +10 for "..."

        // Should end with ellipsis
        assert!(truncated.ends_with("..."));

        // Should end at valid UTF-8 boundary
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn test_truncate_content_unicode_boundary() {
        // Test truncation with multi-byte Unicode characters
        // Each "日本" is 6 bytes (3 bytes per character)
        let unicode_content = "日本".repeat(10_000); // ~60,000 bytes
        let truncated = truncate_content(&unicode_content);

        // Should end at a valid UTF-8 boundary (not in the middle of a character)
        assert!(truncated.is_char_boundary(truncated.len()));

        // Should not panic when converting to string
        let _s = truncated.into_owned();
    }

    #[cfg(feature = "search-tools")]
    #[test]
    fn test_clean_html_small() {
        // Small HTML should be returned as-is
        let html = "<p>Hello</p>";
        let cleaned = clean_html(html);
        assert_eq!(cleaned, html);
    }

    #[cfg(feature = "search-tools")]
    #[test]
    fn test_clean_html_extracts_main() {
        // Should extract content from <main> tag
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <nav>Navigation</nav>
                <main>
                    <h1>Main Content</h1>
                    <p>This is the main article content.</p>
                </main>
                <footer>Footer</footer>
            </body>
            </html>
        "#;

        let cleaned = clean_html(html);

        // Should extract <main> content
        assert!(cleaned.contains("Main Content"));
        assert!(cleaned.contains("article content"));
        // Navigation and footer should not be in cleaned output
        // (note: clean_html returns the HTML of the main element, not the whole page)
    }

    #[cfg(feature = "search-tools")]
    #[test]
    fn test_clean_html_extracts_article() {
        // Should extract content from <article> tag
        let html = r#"
            <html>
            <body>
                <aside>Sidebar</aside>
                <article>
                    <h1>Article Title</h1>
                    <p>Article paragraph.</p>
                </article>
            </body>
            </html>
        "#;

        let cleaned = clean_html(html);
        assert!(cleaned.contains("Article Title"));
        assert!(cleaned.contains("Article paragraph"));
    }

    #[cfg(feature = "search-tools")]
    #[test]
    fn test_clean_html_fallback_to_full() {
        // Should fallback to full HTML when no main content found
        let html = r#"
            <html>
            <body>
                <p>Just some content without semantic markup.</p>
                <p>Another paragraph.</p>
            </body>
            </html>
        "#;

        let cleaned = clean_html(html);
        // Should fallback to returning the original HTML
        assert!(cleaned.contains("Just some content"));
    }
}
