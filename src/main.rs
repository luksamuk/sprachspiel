//! Ask-Ollama: A CLI tool for querying Ollama LLM models
//!
//! This is an evolution of the Python ask-ai.py script, rewritten in Rust
//! with enhanced features including markdown rendering, tool support,
//! model capability detection, and translation support.

mod capabilities;
mod config;
mod ocr;
mod prompts;
mod spinner;
mod summarize;
mod tools;
mod translate;

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
use crate::ocr::{OcrArgs, OcrProcessor, print_results};
use crate::summarize::{SummarizeArgs, SummarizeProcessor};
use crate::translate::{
    Commands, LanguageMapper, TranslateArgs, QueryArgs,
    build_translation_prompt, TranslationStyle, parse_language_pair,
};

/// Type alias for common Result type
type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// CLI for ask-ollama
#[derive(Parser, Debug)]
#[command(
    name = "ask-ollama",
    about = "CLI tool for querying Ollama LLM models and translating text",
    version,
    subcommand_required = false,
    arg_required_else_help = false,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    // Legacy args for backward compatibility when no subcommand is used
    /// The query to send to the model (used when no subcommand specified)
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

    // Handle subcommands if present
    if let Some(command) = cli.command {
        match command {
            Commands::Translate(args) => return handle_translate(args).await,
            Commands::Query(args) => return handle_query(args).await,
            Commands::Ocr(args) => return handle_ocr(args).await,
            Commands::Summarize(args) => return handle_summarize(args).await,
        }
    }

    // No subcommand - handle as legacy query mode for backward compatibility
    handle_legacy_query(cli).await
}

/// Handle translate subcommand
async fn handle_translate(args: TranslateArgs) -> AppResult<()> {
    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let mapper = LanguageMapper::new();

    // Handle --list
    if let Some(filter) = args.list {
        print_supported_languages(&mapper, filter.as_deref());
        return Ok(());
    }

    // Parse language pair
    let language_str = args.language.as_ref().unwrap();
    let (source, target) = match parse_language_pair(language_str, &mapper) {
        Ok((src, tgt)) => (src, tgt),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Get text to translate
    let text = if let Some(text) = args.text {
        text
    } else {
        // Read from stdin
        use std::io::{self, Read};
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        input.trim().to_string()
    };

    if text.is_empty() {
        eprintln!("Error: No text provided for translation.");
        eprintln!("Usage: ask translate LANGUAGE \"text to translate\"");
        eprintln!("   or: echo \"text\" | ask translate LANGUAGE");
        std::process::exit(1);
    }

    // Parse style if provided
    let style = args.prompt.as_ref().map(|s| TranslationStyle::from_str(s));

    // Build translation prompt
    let prompt = build_translation_prompt(source.as_ref(), &target, &text, style.as_ref());

    // Get translate model config
    let model_config = match ModelConfig::get("translate") {
        Some(cfg) => cfg,
        None => {
            eprintln!("Error: Translate model configuration not found.");
            std::process::exit(1);
        }
    };

    // Initialize Ollama client
    let ollama = Ollama::default();

    // Build model options
    let model_options = ModelOptions::default()
        .temperature(model_config.temperature)
        .top_p(model_config.top_p)
        .top_k(model_config.top_k)
        .num_ctx(model_config.num_ctx as u64)
        .repeat_penalty(model_config.repeat_penalty);

    // Build coordinator - no tools for translation
    let mut coordinator = Coordinator::new(ollama, model_config.model_id.clone(), vec![])
        .options(model_options);

    // Create messages - use system prompt for translation instructions
    let system_message = ChatMessage::system(prompt);
    let user_message = ChatMessage::user("".to_string()); // Empty user message since text is in system

    // Show spinner
    let spinner = create_spinner("Translating...");

    // Send request
    let response = coordinator
        .chat(vec![system_message, user_message])
        .await
        .map_err(|e| format!("Failed to get translation: {}", e))?;

    // Clear spinner
    spinner.finish_and_clear();

    // Get translated text
    let translated = response.message.content.trim();

    // Output - always plain for translation
    println!("{}", translated);

    Ok(())
}

/// Handle query subcommand
async fn handle_query(args: QueryArgs) -> AppResult<()> {
    let query = args.get_query()?;
    
    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        std::process::exit(1);
    }

    // Get model configuration
    let model_config = if ModelConfig::is_valid(&args.model) {
        ModelConfig::get(&args.model).unwrap()
    } else {
        eprintln!(
            "Error: Unknown model '{}'. Use --list to see available models.",
            args.model
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
    let use_tools = args.tools || capabilities.tools;

    // Determine if think mode should be enabled
    let use_think = if args.think && capabilities.thinking {
        true
    } else if args.think && !capabilities.thinking {
        eprintln!(
            "Warning: Model '{}' does not support think mode. Ignoring -t flag.",
            model_config.model_id
        );
        false
    } else {
        false
    };

    // Get system prompt
    let prompt_name = if use_tools && args.prompt == "default" {
        "tool_user"
    } else {
        &args.prompt
    };

    let system_prompt = match get_prompt(prompt_name) {
        Some(prompt) => prompt,
        None => {
            eprintln!(
                "Error: Unknown prompt '{}'. Use --list to see available prompts.",
                args.prompt
            );
            std::process::exit(1);
        }
    };

    // Handle debug mode
    if args.debug {
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

    // Show spinner
    let spinner = create_spinner("Waiting for response...");

    // Send request
    let response = coordinator
        .chat(vec![system_message, user_message])
        .await
        .map_err(|e| format!("Failed to get response from Ollama: {}", e))?;

    // Clear spinner
    spinner.finish_and_clear();

    // Strip thinking tags
    let content = strip_thinking_tags(&response.message.content);

    // Render output
    if args.plain {
        println!("{}", content);
    } else {
        print_text(&content);
    }

    Ok(())
}

/// Handle legacy query mode (backward compatibility)
async fn handle_legacy_query(cli: Cli) -> AppResult<()> {
    // Handle --list flag
    if cli.list {
        print_available_options();
        return Ok(());
    }

    // Get query from args or stdin
    let query = get_query_legacy(&cli)?;
    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        eprintln!("Try 'ask-ollama --help' for usage information.");
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

    // Get system prompt
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

    // Show spinner
    let spinner = create_spinner("Waiting for response...");

    // Send request
    let response = coordinator
        .chat(vec![system_message, user_message])
        .await
        .map_err(|e| format!("Failed to get response from Ollama: {}", e))?;

    // Clear spinner
    spinner.finish_and_clear();

    // Strip thinking tags
    let content = strip_thinking_tags(&response.message.content);

    // Render output
    if cli.plain {
        println!("{}", content);
    } else {
        print_text(&content);
    }

    Ok(())
}

/// Print supported languages for translation
fn print_supported_languages(mapper: &LanguageMapper, filter: Option<&str>) {
    let languages = mapper.list(filter);
    
    if languages.is_empty() {
        if let Some(f) = filter {
            println!("No languages found matching '{}'", f);
        } else {
            println!("No languages available.");
        }
        return;
    }

    if let Some(f) = filter {
        println!("Languages matching '{}':", f);
    } else {
        println!("Supported languages (use code or name):");
    }
    println!();
    
    // Group by language family for better display
    let mut current_family = String::new();
    
    for lang in languages {
        // Simple grouping by first two letters of code
        let family = lang.code.split('-').next().unwrap_or(&lang.code).to_string();
        
        if family != current_family {
            if !current_family.is_empty() {
                println!();
            }
            current_family = family;
        }
        
        let aliases_str = if lang.aliases.is_empty() {
            String::new()
        } else {
            format!(" [aliases: {}]", lang.aliases.join(", "))
        };
        
        println!("  {:<15} - {}{}", lang.code, lang.name, aliases_str);
    }
    
    println!();
    println!("Usage examples:");
    println!("  ask translate en:pt \"Hello\"        # English to Portuguese");
    println!("  ask translate :pt \"Hello\"          # Auto-detect to Portuguese");
    println!("  ask translate pt \"Hello\"           # Auto-detect to Portuguese");
    println!("  ask translate he:en \"שלום\"        # Hebrew to English");
    println!("  ask translate en:br \"Hello\"        # English to Brazilian Portuguese");
    println!();
    println!("Tip: Use ambiguous codes like 'zh' or 'pt' for specific variants:");
    println!("  zh-Hans = Chinese Simplified, zh-Hant = Chinese Traditional");
    println!("  pt-BR = Brazilian Portuguese, pt-PT = European Portuguese");
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

    println!("\nSubcommands:");
    println!("  translate [en:pt] TEXT  Translate text between languages");
    println!("  query QUERY             Query an LLM (default if no subcommand)");
    println!();
    println!("Examples:");
    println!("  ask \"What is Rust?\"");
    println!("  ask translate en:pt \"Hello world\"");
    println!("  ask -m lfm \"Explain async/await\"");
    println!("  ask translate --list");
    println!("  ask translate --list port");
}

/// Get query from CLI args or stdin (legacy mode)
fn get_query_legacy(cli: &Cli) -> AppResult<String> {
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

/// Handle OCR subcommand
async fn handle_ocr(args: OcrArgs) -> AppResult<()> {
    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let processor = OcrProcessor::new();

    // Process files
    let results = match processor.process_batch(&args).await {
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Print results
    print_results(&results, args.json);

    Ok(())
}

/// Handle summarize subcommand
async fn handle_summarize(args: SummarizeArgs) -> AppResult<()> {
    // Get text from args or stdin (read once here)
    let text = if let Some(ref text) = args.text {
        text.clone()
    } else {
        // Read from stdin
        use std::io::{self, Read};
        let mut input = String::new();
        match io::stdin().read_to_string(&mut input) {
            Ok(_) => {
                let trimmed = input.trim().to_string();
                if trimmed.is_empty() {
                    eprintln!("Error: No text provided for summarization.");
                    eprintln!("Usage: ask summarize [OPTIONS] <TEXT>");
                    eprintln!("   or: echo \"text\" | ask summarize");
                    std::process::exit(1);
                }
                trimmed
            }
            Err(e) => {
                eprintln!("Error: Failed to read from stdin: {}", e);
                std::process::exit(1);
            }
        }
    };

    let processor = SummarizeProcessor::new();

    // Process summarization with the text already loaded
    match processor.summarize(&args, &text).await {
        Ok(summary) => {
            // Render output with markdown if not --plain
            if args.plain {
                println!("{}", summary);
            } else {
                print_text(&summary);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Strip thinking tags from model output
fn strip_thinking_tags(content: &str) -> String {
    let re = regex::Regex::new(r"(?si)<think>.*?</think>")
        .expect("Invalid regex pattern");
    
    let result = re.replace_all(content, "");
    
    let re2 = regex::Regex::new(r"(?si)<think\s+[^>]*>.*?</think>")
        .expect("Invalid regex pattern 2");
    let result = re2.replace_all(&result, "");
    
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_thinking_tags() {
        let input = "<think>This is thinking</think>Response.";
        let expected = "Response.";
        assert_eq!(strip_thinking_tags(input), expected);

        let input_no_think = "Just a normal response.";
        assert_eq!(strip_thinking_tags(input_no_think), "Just a normal response.");

        let input_multiline = "<think>\nThinking...\n</think>\n\nFinal answer.";
        let expected_multiline = "Final answer.";
        assert_eq!(strip_thinking_tags(input_multiline), expected_multiline);

        let input_upper = "<THINK>Thinking...</THINK>Response.";
        let expected_upper = "Response.";
        assert_eq!(strip_thinking_tags(input_upper), expected_upper);

        let input_multi = "<think>First</think>Part 1. <think>Second</think>Part 2.";
        let expected_multi = "Part 1. Part 2.";
        assert_eq!(strip_thinking_tags(input_multi), expected_multi);
    }
}
