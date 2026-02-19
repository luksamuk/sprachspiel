//! Web search using ollama-rs built-in DDGSearcher
//!
//! Uses DuckDuckGo HTML interface which does not require CAPTCHA.

use crate::debug_tools::{log_tool_call, log_tool_result};
use ollama_rs::function;
use ollama_rs::generation::tools::implementations::DDGSearcher;
use std::sync::Arc;
use tokio::sync::Mutex;

static SEARCHER: once_cell::sync::Lazy<Arc<Mutex<DDGSearcher>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(DDGSearcher::default())));

#[derive(serde::Deserialize)]
struct SearchResult {
    title: String,
    link: String,
    snippet: String,
}

/// Parse optional string to usize with default.
/// Accepts: "5", "10", empty, or None
fn parse_num_results(s: Option<String>, default: usize, max: usize) -> usize {
    match s {
        Some(ref val) if !val.trim().is_empty() => {
            val.trim().parse::<usize>().unwrap_or(default).min(max)
        }
        _ => default,
    }
}

/// Search the web using DuckDuckGo.
///
/// Returns search results with title, URL, and snippet for each result.
/// Use this tool when you need to find current information on the internet.
///
/// * query - The search query (what to search for)
/// * num_results - Number of results to return (default: 5, max: 10)
#[function]
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

    let num_results = parse_num_results(num_results, 5, 10);

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
///
/// * query - The news topic to search for
/// * num_results - Number of results to return (default: 3, max: 10)
#[function]
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

    let num_results = parse_num_results(num_results, 3, 10);
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
/// * url - The URL of the webpage to scrape
#[function]
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

    let content = html2md::parse_html(&html);

    if content.trim().is_empty() {
        let result = format!("No content could be extracted from '{}'.", url);
        log_tool_result("web_scrape", &result);
        return Ok(result);
    }

    let kb = content.len() as f64 / 1024.0;
    let size_info = if kb >= 1024.0 {
        format!(" ({:.1} MB)", kb / 1024.0)
    } else {
        format!(" ({:.0} KB)", kb)
    };

    let result = format!("**Content from {}**{}\n\n{}", url, size_info, content);
    log_tool_result("web_scrape", &result);
    Ok(result)
}
