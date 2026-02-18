//! Test ollama-rs built-in DDGSearcher
//! 
//! Run with: cargo test --package ask-ai --test test_ddg_searcher -- --nocapture

use ollama_rs::generation::tools::implementations::DDGSearcher;

#[tokio::test]
async fn test_ddg_searcher_standalone() {
    let searcher = DDGSearcher::default();
    
    println!("\n=== Testing DDGSearcher from ollama-rs ===");
    println!("Query: 'Rust programming language'");
    
    match searcher.search("Rust programming language").await {
        Ok(results) => {
            println!("SUCCESS! Found {} results", results.len());
            // SearchResult derives Serialize, so we can serialize to see the data
            match serde_json::to_string_pretty(&results) {
                Ok(json) => {
                    println!("\n{}", json);
                }
                Err(e) => {
                    println!("Serialization error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("ERROR: {:?}", e);
            println!("\nNOTE: DDGSearcher may be blocked by CAPTCHA");
        }
    }
}

#[tokio::test]
async fn test_ddg_searcher_simple_query() {
    let searcher = DDGSearcher::default();
    
    println!("\n=== Testing simple query ===");
    println!("Query: 'What is the capital of France'");
    
    match searcher.search("What is the capital of France").await {
        Ok(results) => {
            println!("SUCCESS! Found {} results", results.len());
            if !results.is_empty() {
                match serde_json::to_string_pretty(&results) {
                    Ok(json) => println!("{}", json),
                    Err(e) => println!("Serialization error: {}", e),
                }
            }
        }
        Err(e) => {
            println!("ERROR: {:?}", e);
        }
    }
}