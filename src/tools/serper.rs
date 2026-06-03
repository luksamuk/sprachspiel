//! Web search using Serper.dev API (Google Search)
//!
//! Provides Google Search results via Serper.dev API.
//! Requires SERPER_API_KEY environment variable.

use crate::consts::api::SERPER_API_URL;
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::{parse_bounded_number, post_json_with_headers};
use sprachspiel_tool_derive::tool;
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
struct SerperResponse {
    #[serde(default)]
    organic: Vec<OrganicResult>,
    #[serde(default)]
    news: Vec<NewsResult>,
    #[serde(default)]
    answer_box: Option<AnswerBox>,
}

#[derive(Debug, Deserialize)]
struct OrganicResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    snippet: String,
}

#[derive(Debug, Deserialize)]
struct NewsResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    source: String,
}

#[derive(Debug, Deserialize)]
struct AnswerBox {
    #[serde(default)]
    #[expect(dead_code)]
    title: String,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    snippet: String,
}

fn get_api_key() -> Option<String> {
    std::env::var("SERPER_API_KEY").ok()
}

/// Check if Serper API key is available.
pub fn is_serper_available() -> bool {
    get_api_key().is_some()
}

/// Search the web using Google via Serper.dev API.
///
/// Returns search results with title, URL, and snippet for each result.
/// Use this tool when you need to find current information on the internet.
/// Requires SERPER_API_KEY environment variable to be set.
///
/// # Arguments
/// * `query` - The search query (what to search for). Be specific for better results.
///   - Example: "Rust async programming best practices" instead of just "rust async"
/// * `num_results` - Number of results to return (default: 5, max: 10). Optional.
///
/// # Returns
/// Formatted search results with:
/// - Quick answer (if available from Google)
/// - Title, URL, and snippet for each result
///
/// # Errors
/// Returns error message if SERPER_API_KEY is not set or API fails.
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

    let api_key = match get_api_key() {
        Some(k) => k,
        None => {
            let err = "Error: Serper API key not found. Set SERPER_API_KEY environment variable to enable web search.".to_string();
            log_tool_result("web_search", &err);
            return Ok(err);
        }
    };

    let num_results = parse_bounded_number(num_results.as_deref(), 5, Some(10));

    let data: SerperResponse = match post_json_with_headers(
        SERPER_API_URL,
        "web_search",
        vec![
            ("X-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json"),
        ],
        &serde_json::json!({ "q": &query }),
    )
    .await
    {
        Ok(d) => d,
        Err(e) => return Ok(e),
    };

    if data.organic.is_empty() && data.answer_box.is_none() {
        let result = format!("No results found for '{}'.", query);
        log_tool_result("web_search", &result);
        return Ok(result);
    }

    let mut output = vec![format!("**Search results for '{}'**\n", query)];

    if let Some(ref answer) = data.answer_box {
        if !answer.answer.is_empty() {
            output.push(format!("\n**Quick Answer:** {}\n", answer.answer));
        } else if !answer.snippet.is_empty() {
            output.push(format!("\n**Quick Answer:** {}\n", answer.snippet));
        }
    }

    for (i, r) in data.organic.iter().take(num_results).enumerate() {
        output.push(format!("\n**{}. {}**\n{}", i + 1, r.title, r.link));
        if !r.snippet.is_empty() {
            output.push(format!("\n{}", r.snippet));
        }
    }

    output.push("\n\n_Source: Google Search via Serper_".to_string());

    let result = output.join("\n");
    log_tool_result("web_search", &result);
    Ok(result)
}

/// Search for news using Google via Serper.dev API.
///
/// Searches specifically for news articles about a topic.
///
/// * query - The news topic to search for
/// Search for recent news using Google via Serper.dev API.
///
/// Returns news articles with title, URL, snippet, and publication date.
/// Use this tool when you need current news or recently published information.
/// Requires SERPER_API_KEY environment variable to be set.
///
/// # Arguments
/// * `query` - The news search query. Be specific for better results.
///   - Example: "AI technology latest news" or "Rust programming updates"
/// * `num_results` - Number of results to return (default: 3, max: 10). Optional.
///
/// # Returns
/// Formatted news results with:
/// - Article title
/// - Source and publication date
/// - URL and snippet
///
/// # Errors
/// Returns error message if SERPER_API_KEY is not set or API fails.
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

    let api_key = match get_api_key() {
        Some(k) => k,
        None => {
            let err = "Error: Serper API key not found. Set SERPER_API_KEY environment variable to enable news search.".to_string();
            log_tool_result("web_search_news", &err);
            return Ok(err);
        }
    };

    let num_results = parse_bounded_number(num_results.as_deref(), 3, Some(10));

    let data: SerperResponse = match post_json_with_headers(
        SERPER_API_URL,
        "web_search_news",
        vec![
            ("X-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json"),
        ],
        &serde_json::json!({ "q": &query, "type": "news" }),
    )
    .await
    {
        Ok(d) => d,
        Err(e) => return Ok(e),
    };

    if data.news.is_empty() {
        let result = format!("No news found for '{}'.", query);
        log_tool_result("web_search_news", &result);
        return Ok(result);
    }

    let mut output = vec![format!("**News about '{}'**\n", query)];

    for (i, r) in data.news.iter().take(num_results).enumerate() {
        output.push(format!("\n**{}. {}**\n{}", i + 1, r.title, r.link));
        if !r.snippet.is_empty() {
            output.push(format!("\n{}", r.snippet));
        }
        if !r.date.is_empty() || !r.source.is_empty() {
            output.push(format!("\n_{} - {}_", r.source, r.date));
        }
    }

    output.push("\n\n_Source: Google News via Serper_".to_string());

    let result = output.join("\n");
    log_tool_result("web_search_news", &result);
    Ok(result)
}
