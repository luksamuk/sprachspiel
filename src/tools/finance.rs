//! Stock quote tools using Google Finance scraper
//!
//! Provides stock information via Google Finance web scraping.

use crate::debug_tools::{log_tool_call, log_tool_result};
use sprachspiel_tool_derive::tool;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use scraper::{Html, Selector};

static STOCK_SCRAPER: Lazy<Arc<Mutex<StockScraper>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(StockScraper::new())));

struct StockScraper {
    base_url: String,
    language: String,
}

impl StockScraper {
    fn new() -> Self {
        StockScraper {
            base_url: "https://www.google.com/finance".to_string(),
            language: "en".to_string(),
        }
    }

    async fn scrape(
        &self,
        exchange: &str,
        ticker: &str,
    ) -> Result<HashMap<String, String>, String> {
        let target_url = format!(
            "{}/quote/{}:{}?hl={}",
            self.base_url, ticker, exchange, self.language
        );

        let client = match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .build()
        {
            Ok(c) => c,
            Err(e) => return Err(format!("Error creating HTTP client: {}", e)),
        };

        let response = match client.get(&target_url).send().await {
            Ok(r) => r,
            Err(e) => return Err(format!("Network error: {}", e)),
        };

        let content = match response.text().await {
            Ok(c) => c,
            Err(e) => return Err(format!("Error reading response: {}", e)),
        };

        let document = Html::parse_document(&content);

        let items_selector = match Selector::parse("div.gyFHrc") {
            Ok(s) => s,
            Err(_) => return Err("Failed to parse items selector".to_string()),
        };

        let desc_selector = match Selector::parse("div.mfs7Fc") {
            Ok(s) => s,
            Err(_) => return Err("Failed to parse description selector".to_string()),
        };

        let value_selector = match Selector::parse("div.P6K39c") {
            Ok(s) => s,
            Err(_) => return Err("Failed to parse value selector".to_string()),
        };

        let mut stock_data = HashMap::new();

        for item in document.select(&items_selector) {
            if let Some(item_desc) = item.select(&desc_selector).next()
                && let Some(item_val) = item.select(&value_selector).next()
            {
                stock_data.insert(
                    item_desc.text().collect::<Vec<_>>().join(""),
                    item_val.text().collect::<Vec<_>>().join(""),
                );
            }
        }

        if stock_data.is_empty() {
            return Err(format!(
                "No data found for ticker {} on exchange {}. The stock symbol may be incorrect or not available on Google Finance.",
                ticker, exchange
            ));
        }

        Ok(stock_data)
    }
}

/// Get stock quote information from Google Finance.
///
/// Returns current stock data including price, market cap, P/E ratio, etc.
/// Common exchange codes: NASDAQ, NYSE, BVMF (Brazil), LON (London), TPE (Tokyo)
/// Example tickers: AAPL (Apple), GOOGL (Google), PETR4 (Petrobras on BVMF)
///
/// * exchange - The stock exchange MIC code (e.g., "NASDAQ", "NYSE", "BVMF")
/// * ticker - The stock ticker symbol (e.g., "AAPL", "GOOGL", "PETR4")
#[tool]
pub async fn get_stock_quote(
    exchange: String,
    ticker: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "get_stock_quote",
        &[
            ("exchange".to_string(), exchange.clone()),
            ("ticker".to_string(), ticker.clone()),
        ],
    );

    let scraper = STOCK_SCRAPER.clone();
    let scraper = scraper.lock().await;

    let data = match scraper.scrape(&exchange, &ticker).await {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error: {}", e);
            log_tool_result("get_stock_quote", &err);
            return Ok(err);
        }
    };

    let mut output = vec![format!("**Stock Quote: {}:{}**\n", exchange, ticker)];

    let mut sorted_data: Vec<_> = data.iter().collect();
    sorted_data.sort_by_key(|(k, _)| *k);

    for (key, value) in sorted_data {
        output.push(format!("- {}: {}", key, value));
    }

    output.push("\n_Source: Google Finance_".to_string());

    let result = output.join("\n");
    log_tool_result("get_stock_quote", &result);
    Ok(result)
}
