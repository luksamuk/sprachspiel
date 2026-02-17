mod config;
mod prompts;
mod tools;

use clap::Parser;
use ollama_rs::Ollama;
use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;

use crate::config::ModelConfig;
use crate::prompts::get_prompt;
use crate::tools::*;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Sync + Send>>;

#[derive(Parser, Debug)]
#[command(name = "ask-ollama")]
#[command(about = "CLI tool for querying Ollama LLM models")]
#[command(version)]
struct Cli {
    /// The query to send to the model
    #[arg(value_name = "QUERY")]
    query: String,

    /// Model preset to use (llama, qwen, mistral, lfm)
    #[arg(short, long, default_value = "lfm", value_name = "MODEL")]
    model: String,

    /// System prompt mode (concise, tool_user)
    #[arg(short, long, default_value = "tool_user", value_name = "PROMPT")]
    prompt: String,

    /// Temperature for the model (0.0 - 2.0)
    #[arg(short, long, value_name = "TEMP")]
    temperature: Option<f32>,

    /// List available models and prompts
    #[arg(short, long)]
    list: bool,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();

    if cli.list {
        println!("Available models:");
        for name in ModelConfig::list_names() {
            if let Some(config) = ModelConfig::get(name) {
                println!("  {}: {}", name, config.model_id);
            }
        }
        println!("\nAvailable prompts:");
        for name in prompts::list_prompts() {
            println!("  {}", name);
        }
        return Ok(());
    }

    let model_config = ModelConfig::get(&cli.model).ok_or_else(|| {
        format!(
            "Unknown model: {}. Use --list to see available models.",
            cli.model
        )
    })?;

    let system_prompt = get_prompt(&cli.prompt).ok_or_else(|| {
        format!(
            "Unknown prompt: {}. Use --list to see available prompts.",
            cli.prompt
        )
    })?;

    let temperature = cli.temperature.unwrap_or(model_config.temperature);

    let ollama = Ollama::default();

    let model_options = ModelOptions::default()
        .temperature(temperature)
        .top_p(model_config.top_p)
        .top_k(model_config.top_k)
        .num_ctx(model_config.num_ctx as u64)
        .repeat_penalty(model_config.repeat_penalty);

    let mut coordinator = Coordinator::new(ollama, model_config.model_id.clone(), vec![])
        .options(model_options)
        .think(false);

    coordinator = coordinator
        .add_tool(fetch_pokemon)
        .add_tool(fetch_pokemon_basic)
        .add_tool(fetch_pokemon_stats)
        .add_tool(fetch_pokemon_moves)
        .add_tool(fetch_pokemon_evolution)
        .add_tool(fetch_ability_details)
        .add_tool(fetch_type_effectiveness)
        .add_tool(fetch_move_details);

    println!("{}", "=".repeat(60));
    println!("USER PROMPT:");
    println!("{}", "=".repeat(60));
    println!("{}", cli.query);
    println!("{}", "=".repeat(60));
    println!();

    let system_message = ChatMessage::system(system_prompt.to_string());
    let user_message = ChatMessage::user(cli.query);

    let response = coordinator.chat(vec![system_message, user_message]).await?;

    println!("{}", response.message.content);

    Ok(())
}
