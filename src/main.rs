//! Ask-Ollama: A CLI tool for querying Ollama LLM models
//!
//! This is an evolution of the Python ask-ai.py script, rewritten in Rust
//! with enhanced features including markdown rendering, tool support,
//! and model capability detection.

mod capabilities;
mod config;
mod prompts;
mod spinner;
mod tools;

use clap::Parser;
use ollama_rs::Ollama;
use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;
use termimad::print_text;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::prompts::get_prompt;
use crate::spinner::create_spinner;
use crate::tools::*;

/// Type alias for common Result type
type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// CLI arguments for ask-ollama
#[derive(Parser, Debug)]
#[command(
    name = "ask-ollama",
    about = "CLI tool for querying Ollama LLM models",
    version,
    after_help = "
Examples:
  ask-ollama \"Write a Rust function for Fibonacci\"
  ask-ollama -m qwen3-coder \"Generate an Adler32 hash function in C\"
  ask-ollama -t -m glm-4.7-flash \"Explain Rust async patterns\"
  cat README.md | ask-ollama -m smollm3 \"Summarize this\"
  ask-ollama \"Tell me about Pikachu\"  # Auto-enables tools if model supports
  ask-ollama --plain \"List Rust keywords\"
"
)]
struct Cli {
    /// The query to send to the model (optional, falls back to stdin)
    #[arg(value_name = "QUERY")]
    query: Option<String>,

    /// Model preset to use (lfm is the default)
    #[arg(short, long, default_value = "lfm", value_name = "MODEL")]
    model: String,

    /// System prompt mode (default, tool_user)
    #[arg(short, long, default_value = "default", value_name = "PROMPT")]
    prompt: String,

    /// Enable think mode for models that support it
    #[arg(short, long)]
    think: bool,

    /// Output plain text without markdown formatting
    #[arg(long)]
    plain: bool,

    /// Dry-run mode: print config without executing
    #[arg(short, long)]
    debug: bool,

    /// List available models and prompts
    #[arg(short, long)]
    list: bool,

    /// Force enable tools even if model doesn't advertise tool support
    #[arg(long)]
    tools: bool,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();

    // Handle --list flag
    if cli.list {
        print_available_options();
        return Ok(());
    }

    // Get query from args or stdin
    let query = get_query(&cli)?;
    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        std::process::exit(1);
    }

    // Get model configuration
    let model_config = if ModelConfig::is_valid(&cli.model) {
        ModelConfig::get(&cli.model).unwrap()
    } else {
        eprintln!(
            "Error: Unknown model '{}'. Use --list to see available models.",
            cli.model
        );
        std::process::exit(1);
    };

    // Initialize Ollama client
    let ollama = Ollama::default();

    // Detect model capabilities
    let capabilities = match ModelCapabilities::detect(&ollama, &model_config.model_id).await {
        Ok(caps) => caps,
        Err(e) => {
            eprintln!("Warning: Could not detect model capabilities: {}", e);
            eprintln!("Continuing without capability detection...");
            ModelCapabilities {
                tools: false,
                vision: false,
                completion: true,
                thinking: false,
            }
        }
    };

    // Determine if tools should be enabled
    let use_tools = cli.tools || capabilities.tools;

    // Determine if think mode should be enabled
    let use_think = if cli.think && capabilities.thinking {
        true
    } else if cli.think && !capabilities.thinking {
        eprintln!(
            "Warning: Model '{}' does not support think mode. Ignoring -t flag.",
            model_config.model_id
        );
        false
    } else {
        false
    };

    // Get system prompt (use tool_user if tools are enabled and not explicitly overridden)
    let prompt_name = if use_tools && cli.prompt == "default" {
        "tool_user"
    } else {
        &cli.prompt
    };

    let system_prompt = match get_prompt(prompt_name) {
        Some(prompt) => prompt,
        None => {
            eprintln!(
                "Error: Unknown prompt '{}'. Use --list to see available prompts.",
                cli.prompt
            );
            std::process::exit(1);
        }
    };

    // Handle debug mode
    if cli.debug {
        print_debug_info(
            &model_config,
            &capabilities,
            use_tools,
            use_think,
            &query,
            prompt_name,
        );
        return Ok(());
    }

    // Build model options
    let model_options = ModelOptions::default()
        .temperature(model_config.temperature)
        .top_p(model_config.top_p)
        .top_k(model_config.top_k)
        .num_ctx(model_config.num_ctx as u64)
        .repeat_penalty(model_config.repeat_penalty);

    // Build coordinator
    let mut coordinator = Coordinator::new(ollama, model_config.model_id.clone(), vec![])
        .options(model_options)
        .think(use_think);

    // Add tools if enabled
    if use_tools {
        coordinator = coordinator
            .add_tool(fetch_pokemon)
            .add_tool(fetch_pokemon_basic)
            .add_tool(fetch_pokemon_stats)
            .add_tool(fetch_pokemon_moves)
            .add_tool(fetch_pokemon_evolution)
            .add_tool(fetch_ability_details)
            .add_tool(fetch_type_effectiveness)
            .add_tool(fetch_move_details);
    }

    // Create messages
    let system_message = ChatMessage::system(system_prompt.to_string());
    let user_message = ChatMessage::user(query);

    // Show spinner while waiting
    let spinner = create_spinner("Waiting for response...");

    // Send request
    let response = coordinator
        .chat(vec![system_message, user_message])
        .await
        .map_err(|e| format!("Failed to get response from Ollama: {}", e))?;

    // Clear spinner
    spinner.finish_and_clear();

    // Render output
    if cli.plain {
        println!("{}", response.message.content);
    } else {
        print_text(&response.message.content);
    }

    Ok(())
}

/// Print available models and prompts
fn print_available_options() {
    println!("Available models:");
    for name in ModelConfig::list_names() {
        if let Some(config) = ModelConfig::get(name) {
            let default_marker = if name == "lfm" { " (default)" } else { "" };
            println!(
                "  {:20} - {} ({}K context){}",
                name,
                config.model_id,
                config.num_ctx / 1024,
                default_marker
            );
        }
    }

    println!("\nAvailable prompts:");
    for name in prompts::list_prompts() {
        let default_marker = if name == "default" { " (default)" } else { "" };
        println!("  {:20}{}", name, default_marker);
    }

    println!("\nCapabilities:");
    println!("  Models can advertise: tools, vision, completion, thinking");
    println!("  Use --tools to force tool mode even if not advertised");
    println!("  Use -t/--think to enable thinking for thinking-capable models");
}

/// Get query from CLI args or stdin
fn get_query(cli: &Cli) -> AppResult<String> {
    // First check if query was provided as positional argument
    if let Some(ref query) = cli.query {
        return Ok(query.trim().to_string());
    }

    // Otherwise, try to read from stdin
    use std::io::{self, Read};
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    Ok(input.trim().to_string())
}

/// Print debug information
fn print_debug_info(
    model_config: &ModelConfig,
    capabilities: &ModelCapabilities,
    use_tools: bool,
    use_think: bool,
    query: &str,
    prompt_name: &str,
) {
    println!("Debug Mode - Configuration:");
    println!("==========================");
    println!("Model ID:          {}", model_config.model_id);
    println!("Context Window:    {}K tokens", model_config.num_ctx / 1024);
    println!("Temperature:       {}", model_config.temperature);
    println!("Top K:             {}", model_config.top_k);
    println!("Top P:             {}", model_config.top_p);
    println!("Repeat Penalty:    {}", model_config.repeat_penalty);
    println!();
    println!("Detected Capabilities:");
    println!("  Tools:      {}", capabilities.tools);
    println!("  Vision:     {}", capabilities.vision);
    println!("  Completion: {}", capabilities.completion);
    println!("  Thinking:   {}", capabilities.thinking);
    println!();
    println!("Active Configuration:");
    println!("  Tools Enabled:   {}", use_tools);
    println!("  Think Mode:      {}", use_think);
    println!("  Prompt Mode:     {}", prompt_name);
    println!();
    println!("Query: {}", query);
    println!("==========================");
    println!("Dry-run complete. No request was made to Ollama.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test that CLI parsing works
        let cli = Cli::parse_from(["ask-ollama", "--help"]);
        // This will print help and exit, so we just verify it doesn't panic
    }

    #[test]
    fn test_default_values() {
        let cli = Cli::parse_from(["ask-ollama", "test query"]);
        assert_eq!(cli.model, "pepe");
        assert_eq!(cli.prompt, "default");
        assert!(!cli.think);
        assert!(!cli.plain);
        assert!(!cli.debug);
        assert!(!cli.tools);
    }
}
