//! Web search tool using DuckDuckGo Lite (free, no API key required)
//!
//! Performs web searches using DuckDuckGo Lite (html.duckduckgo.com) endpoint
//! which returns HTML that can be parsed for search results.

use crate::debug_tools::{log_tool_call, log_tool_result};
use ollama_rs::function;

/// DuckDuckGo Lite base URL
const DUCKDUCKGO_LITE: &str = "https://html.duckduckgo.com/html/";

/// Internal implementation of web search
async fn do_web_search(
    query: String,
    num_results: Option<u8>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Log tool call with arguments
    log_tool_call("web_search", &[
        ("query".to_string(), query.clone()),
        ("num_results".to_string(), format!("{:?}", num_results)),
    ]);
    
    let num_results = num_results.unwrap_or(5).min(10) as usize;

    // Encode the query
    let encoded_query = urlencoding::encode(&query);
    let url = format!("{}?q={}&kl=wt-wt", DUCKDUCKGO_LITE, encoded_query);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("DuckDuckGo error: {}", response.status()).into());
    }

    let html: String = response.text().await?;

    // Check if DuckDuckGo is blocking with CAPTCHA
    if html.contains("Unfortunately, bots use DuckDuckGo too") ||
       html.contains("anomaly-modal__title") ||
       html.contains("anomaly-modal__description") {
        return Ok(
            "⚠️ **Aviso: DuckDuckGo bloqueou a requisição**\n\n\
            O DuckDuckGo detectou que esta é uma requisição automatizada e exigiu verificação CAPTCHA. \
            Infelizmente, não é possível completar a pesquisa no momento.\n\n\
            **Sugestão:** Tente usar outros modelos que possam responder baseados em seu conhecimento, \
            ou espere que implementemos uma alternativa de busca.".to_string()
        );
    }

    // Parse search results from HTML
    let results = parse_search_results(&html, num_results)?;

    if results.is_empty() {
        let result = format!("Nenhum resultado encontrado para '{}'", query);
        log_tool_result("web_search", &result);
        return Ok(result);
    }

    // Format results
    let mut output = vec![format!("**Resultados da busca por '{}'**\n", query)];

    for (i, result) in results.iter().enumerate() {
        output.push(format!("\n**{}. {}**\n{}", i + 1, result.title, result.url,));
        if !result.snippet.is_empty() {
            output.push(format!("\n{}", result.snippet));
        }
    }

    output.push("\n\n_Fonte: DuckDuckGo_".to_string());

    let result = output.join("\n");
    log_tool_result("web_search", &result);
    Ok(result)
}

/// Perform a web search using DuckDuckGo Lite
///
/// Returns search results with title, URL, and snippet for each result
#[function]
pub async fn web_search(
    query: String,
    num_results: Option<u8>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    do_web_search(query, num_results).await
}

/// Internal implementation of news search
async fn do_web_search_news(
    query: String,
    num_results: Option<u8>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let num_results = num_results.unwrap_or(3).min(10) as usize;
    let news_query = format!("{} news", query);

    let encoded_query = urlencoding::encode(&news_query);
    let url = format!("{}?q={}&kl=wt-wt", DUCKDUCKGO_LITE, encoded_query);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("DuckDuckGo error: {}", response.status()).into());
    }

    let html: String = response.text().await?;
    let results = parse_search_results(&html, num_results)?;

    if results.is_empty() {
        return Ok(format!("Nenhuma notícia encontrada para '{}'", query));
    }

    let mut output = vec![format!("**Notícias sobre '{}'**\n", query)];

    for (i, result) in results.iter().enumerate() {
        output.push(format!("\n**{}. {}**\n{}", i + 1, result.title, result.url,));
        if !result.snippet.is_empty() {
            output.push(format!("\n{}", result.snippet));
        }
    }

    output.push("\n\n_Fonte: DuckDuckGo News_".to_string());

    Ok(output.join("\n"))
}

/// Search for news using DuckDuckGo
///
/// Searches specifically for news articles
#[function]
pub async fn web_search_news(
    query: String,
    num_results: Option<u8>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    do_web_search_news(query, num_results).await
}

/// Internal implementation of instant answer
async fn do_web_instant_answer(
    query: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let encoded_query = urlencoding::encode(&query);
    let url = format!("{}?q={}&kl=wt-wt", DUCKDUCKGO_LITE, encoded_query);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("DuckDuckGo error: {}", response.status()).into());
    }

    let html: String = response.text().await?;

    // Try to extract instant answer (Zero-Click Info)
    if let Some(answer) = extract_instant_answer(&html) {
        return Ok(format!(
            "**{}**\n\n{}\n\n_Fonte: DuckDuckGo_",
            query, answer
        ));
    }

    // If no instant answer, fall back to regular search
    do_web_search(query, Some(3)).await
}

/// Get instant answer for a query (if available)
///
/// Useful for quick facts, definitions, calculations
#[function]
pub async fn web_instant_answer(
    query: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    do_web_instant_answer(query).await
}

/// Structure for search result
#[derive(Debug)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Parse search results from DuckDuckGo Lite HTML
fn parse_search_results(
    html: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
    let mut results = Vec::new();

    // Split by result class to find each result
    // Each result is in a div with class "result"
    let chunks: Vec<&str> = html.split("class=\"result\"").collect();

    for (_i, chunk) in chunks.iter().enumerate().take(max_results + 1).skip(1) {
        if let Some(result) = parse_single_result(chunk) {
            results.push(result);
        }
    }

    Ok(results)
}

/// Parse a single result block
fn parse_single_result(html: &str) -> Option<SearchResult> {
    // Extract title - it's in a link with class "result__a"
    let title = extract_between(html, "class=\"result__a\"", "</a>")?;
    // The title is after the > that closes the tag
    let title = title.rsplit_once('>').map(|(_, t)| t).unwrap_or(&title);
    let title = decode_html_entities(title);

    // Extract URL - it's in the href attribute
    let url_start = html.find("href=\"")? + 7;
    let url_end = html[url_start..].find("\"")?;
    let url = html[url_start..url_start + url_end].to_string();

    // Extract snippet - it's in class result__snippet
    let snippet = extract_between(html, "class=\"result__snippet\">", "</a>")
        .map(|s| decode_html_entities(&s))
        .unwrap_or_default();

    Some(SearchResult {
        title: title.trim().to_string(),
        url: decode_html_entities(&url),
        snippet: snippet.trim().to_string(),
    })
}

/// Extract text between two delimiters
fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = text.find(start)? + start.len();
    let remaining = &text[start_idx..];
    let end_idx = remaining.find(end)?;
    Some(remaining[..end_idx].to_string())
}

/// Extract instant answer from DuckDuckGo HTML
fn extract_instant_answer(html: &str) -> Option<String> {
    // Look for abstract class
    if let Some(answer) = extract_between(html, "class=\"result__abstract\">", "</div>") {
        let clean = decode_html_entities(&answer);
        let clean = clean.trim().to_string();
        if !clean.is_empty() {
            return Some(clean);
        }
    }

    None
}

/// Decode common HTML entities
fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
        .replace("&#39;", "'")
        .replace("&#47;", "/")
        .replace("\n", " ")
        .replace("\r", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_between() {
        let text = r#"class="result__a" href="http://example.com">Hello World</a>"#;
        assert_eq!(
            extract_between(text, r#"class="result__a""#, "</a>"),
            Some(r#" href="http://example.com">Hello World"#.to_string())
        );
    }

    #[test]
    fn test_html_entities() {
        assert_eq!(decode_html_entities("Hello &amp; World"), "Hello & World");
        assert_eq!(decode_html_entities("1 &lt; 2"), "1 < 2");
    }
}
