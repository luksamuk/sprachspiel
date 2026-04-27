//! Ask-Ollama: A CLI tool for querying Ollama LLM models
//!
//! This is an evolution of the Python ask-ai.py script, rewritten in Rust
//! with enhanced features including markdown rendering, tool support,
//! model capability detection, and translation support.

mod capabilities;
mod chat;
mod config;
mod consts;
mod content;
mod context;
mod context_overflow;
mod db;
mod debug_tools;
mod embeddings;
pub mod external;
mod facts;
mod feedback;
pub mod logging;
mod macros;
mod markdown;
mod ocr;
mod platform;
mod project;
mod prompts;
mod query;
mod retrieval;
mod security;
mod settings;
mod skills;
mod soul;
mod spinner;
mod summarize;
mod tokens;
mod tool_robustness;
mod tools;
mod translate;
mod user_models;
mod utils;
mod vision;

use clap::Parser;
use ollama_rs::generation::chat::ChatMessage;

use crate::chat::ChatArgs;
use crate::ocr::mode::is_glm_ocr_model;
use crate::ocr::{OcrArgs, OcrProcessor, print_results as print_ocr_results};
use crate::query::{OutputFlags, run_query};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::summarize::{SummarizeArgs, SummarizeProcessor};
use crate::translate::{
    Commands, CompletionArgs, LanguageMapper, QueryArgs, Shell, TranslateArgs, TranslationStyle,
    build_translation_prompt, parse_language_pair,
};
use crate::vision::{VisionArgs, VisionProcessor, print_results as print_vision_results};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// CLI for ask-ai
#[derive(Parser, Debug)]
#[command(
    name = "ask-ai",
    bin_name = "ask-ai",
    about = "CLI tool for querying Ollama LLM models and translating text",
    version,
    subcommand_required = false,
    arg_required_else_help = false
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// The query to send to the model (used when no subcommand specified)
    #[arg(value_name = "QUERY")]
    query: Option<String>,

    /// Model preset to use (lfm is the default)
    #[arg(short, long, value_name = "MODEL")]
    model: Option<String>,

    /// System prompt mode (default, tool_user)
    #[arg(short, long, default_value = "default", value_name = "PROMPT")]
    prompt: String,

    /// Enable think mode for models that support it
    #[arg(short, long)]
    think: bool,

    /// Output plain text without markdown formatting
    #[arg(long, action = clap::ArgAction::SetTrue)]
    plain: Option<bool>,

    /// List available models and prompts
    #[arg(short, long)]
    list: bool,

    /// Force enable tools even if model doesn't advertise tool support
    #[arg(long)]
    tools: bool,

    /// Code mode: optimize response for code output (minimal explanations)
    #[arg(short, long)]
    code: bool,

    /// Ignore AGENTS.md file if present in current directory
    #[arg(long)]
    ignore_agents: bool,

    /// Skip SOUL.md personality (use neutral personality)
    #[arg(long)]
    soulless: bool,

    /// Increase verbosity (-v for verbose/debug, -vv for trace)
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Quiet mode — only errors and the final answer (no spinner, no tool calls)
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Initialize/create sample configuration file
    #[arg(long)]
    init_config: bool,

    /// Custom database path (overrides default ~/.local/share/ask-ai/ask-ai.db)
    #[arg(long, value_name = "PATH")]
    db: Option<String>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize sqlite-vec extension globally
    // This must be done before any database operations
    crate::db::init();

    let cli = Cli::parse();

    if cli.init_config {
        match Settings::create_sample_config() {
            Ok(path) => {
                println!("Configuration file created at: {}", path.display());
                println!("\nEdit this file to customize your Ask-AI settings.");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error creating configuration file: {}", e);
                std::process::exit(1);
            }
        }
    }

    let settings = Settings::load();

    // Initialize logging system based on CLI flags, RUST_LOG, and config
    // Priority: CLI flags > RUST_LOG env var > config.toml > default (info)
    let config_verbosity = settings.output.verbosity;
    let verbosity = crate::logging::Verbosity::resolve(cli.quiet, cli.verbose, config_verbosity);
    crate::logging::init(verbosity);

    // Initialize markdown skin with user configuration
    markdown::init_markdown_skin(&settings.display.skin);

    if let Some(ref command) = cli.command {
        match command {
            Commands::Translate(args) => {
                return handle_translate(args.clone(), &cli, &settings).await;
            }
            Commands::Query(args) => {
                return handle_query_subcommand(args.clone(), &cli, &settings).await;
            }
            Commands::Ocr(args) => return handle_ocr(args.clone(), &cli, &settings).await,
            Commands::Summarize(args) => {
                return handle_summarize(args.clone(), &cli, &settings).await;
            }
            Commands::Chat(args) => return handle_chat(args.clone(), &cli, &settings).await,
            Commands::Vision(args) => return handle_vision(args.clone(), &cli, &settings).await,
            Commands::Completion(args) => return handle_completion(args.clone(), &settings),
        }
    }

    handle_legacy_query(cli, &settings).await
}

async fn handle_translate(args: TranslateArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let output_flags = OutputFlags::resolve(cli.plain);

    log::debug!("Debug Mode - Translation Configuration:");
    log::debug!("==========================");
    if let Some(lang) = &args.language {
        log::debug!("Language:          {}", lang);
    }
    if let Some(text) = &args.text {
        let preview = crate::utils::truncate_chars(text, 50);
        log::debug!("Text:              {}", preview);
    }
    log::debug!("==========================");
    log::debug!("Executing translation with logging enabled...");

    let mapper = LanguageMapper::new();

    if let Some(filter) = args.list {
        print_supported_languages(&mapper, filter.as_deref());
        return Ok(());
    }

    let language_str = args.language.as_ref().unwrap();
    let (source, target) = match parse_language_pair(language_str, &mapper) {
        Ok((src, tgt)) => (src, tgt),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let text = if let Some(text) = args.text {
        text
    } else {
        match crate::utils::read_stdin() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("Usage: ask translate LANGUAGE \"text to translate\"");
                eprintln!("   or: echo \"text\" | ask translate LANGUAGE");
                std::process::exit(1);
            }
        }
    };

    if text.is_empty() {
        eprintln!("Error: No text provided for translation.");
        eprintln!("Usage: ask translate LANGUAGE \"text to translate\"");
        eprintln!("   or: echo \"text\" | ask translate LANGUAGE");
        std::process::exit(1);
    }

    let style = args.prompt.as_ref().map(|s| TranslationStyle::parse(s));
    let prompt = build_translation_prompt(source.as_ref(), &target, &text, style.as_ref());

    // Get translate model from settings or fall back to builtin "translategemma"
    // Priority: settings.model.translate.model -> "translategemma" (NOT global default)
    let translate_model = settings
        .model
        .translate
        .model
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "translategemma".to_string());

    let model_config = match user_models::get_model_config(&translate_model) {
        Some(cfg) => cfg,
        None => {
            eprintln!(
                "Error: Translate model '{}' not found. \
                 Add it to ~/.config/ask-ai/models.toml or use a built-in model.",
                translate_model
            );
            eprintln!("Built-in models: qwen3.5:4b, translategemma, glm-ocr");
            std::process::exit(1);
        }
    };

    let ollama = settings.ollama_client();
    let model_options = model_config.build_model_options();

    let mut coordinator =
        chat::CustomCoordinator::new(ollama, model_config.model_id.clone(), vec![])
            .options(model_options);

    let system_message = ChatMessage::system(prompt);
    let user_message = ChatMessage::user("".to_string());

    let spinner = create_spinner("Translating...");

    let response = coordinator
        .chat(vec![system_message, user_message])
        .await
        .map_err(|e| format!("Failed to get translation: {}", e))?;

    finish_spinner(spinner);

    let translated = response.message.content.trim();

    if output_flags.plain {
        println!("{}", translated);
    } else {
        markdown::print_markdown(translated);
    }

    Ok(())
}

async fn handle_query_subcommand(args: QueryArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    let query = args.get_query()?;

    run_query(
        query,
        cli.model.as_deref(),
        cli.think,
        cli.tools,
        cli.code,
        &cli.prompt,
        cli.ignore_agents,
        cli.soulless,
        cli.plain,
        settings,
    )
    .await
}

async fn handle_legacy_query(cli: Cli, settings: &Settings) -> AppResult<()> {
    if cli.list {
        print_available_options();
        return Ok(());
    }

    let query = get_query_legacy(&cli)?;
    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        eprintln!("Try 'ask-ai --help' for usage information.");
        std::process::exit(1);
    }

    run_query(
        query,
        cli.model.as_deref(),
        cli.think,
        cli.tools,
        cli.code,
        &cli.prompt,
        cli.ignore_agents,
        cli.soulless,
        cli.plain,
        settings,
    )
    .await
}

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

    let mut current_family = String::new();

    for lang in languages {
        let family = lang
            .code
            .split('-')
            .next()
            .unwrap_or(&lang.code)
            .to_string();

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

fn print_available_options() {
    println!("Available models:");
    for name in user_models::list_all_model_names() {
        if let Some(config) = user_models::get_model_config(&name) {
            let default_marker = if name == "qwen3.5:4b" {
                " (default)"
            } else {
                ""
            };
            let user_marker = if !config::ModelConfig::is_builtin_valid(&name) {
                " [user]"
            } else {
                ""
            };
            println!(
                "  {:20} - {} ({}K context){}{}",
                name,
                config.model_id,
                config.num_ctx / 1024,
                default_marker,
                user_marker
            );
        }
    }

    println!("\nAvailable prompts:");
    for name in prompts::list_prompts() {
        let default_marker = if name == "default" { " (default)" } else { "" };
        let special_marker = if name == "pepe" {
            " (Easter egg: Pepe personality)"
        } else {
            ""
        };
        println!("  {:20}{}{}", name, default_marker, special_marker);
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

fn get_query_legacy(cli: &Cli) -> AppResult<String> {
    if let Some(ref query) = cli.query {
        return Ok(query.trim().to_string());
    }

    use std::io::{self, Read};
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    Ok(input.trim().to_string())
}

async fn handle_ocr(args: OcrArgs, _cli: &Cli, settings: &Settings) -> AppResult<()> {
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Security: validate all file paths against blocklist + CWD sandbox
    let expanded_paths: Vec<std::path::PathBuf> = args
        .files
        .iter()
        .map(|p| crate::utils::expand_tilde_path(&p.to_string_lossy()))
        .collect();
    if let Err(e) = crate::security::validate_subagent_paths(&expanded_paths) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let (model_key, _, _) = settings.get_subcommand_config("ocr");
    let (model_id, model_options) = crate::user_models::get_model_config(&model_key)
        .map(|mc| (mc.model_id.clone(), mc.build_model_options()))
        .unwrap_or_else(|| {
            (
                model_key.clone(),
                ollama_rs::models::ModelOptions::default().temperature(0.0),
            )
        });

    log::debug!("Debug Mode - OCR Configuration:");
    log::debug!("==========================");
    log::debug!("Model ID:          {}", model_id);
    log::debug!("Mode:              {:?}", args.mode);
    log::debug!("Max Tokens:        {}", args.max_tokens);
    log::debug!("JSON Output:       {}", args.json);
    log::debug!("Files:             {:?}", args.files);
    log::debug!("==========================");
    log::debug!("Executing OCR with logging enabled...");

    let processor = OcrProcessor::new();
    let ollama = settings.ollama_client();

    let prompt_override = if is_glm_ocr_model(&model_id) {
        None
    } else {
        Some(args.mode.into_descriptive_prompt())
    };

    let results = match processor
        .process_batch(
            &args,
            prompt_override,
            &model_id,
            model_options,
            &ollama,
            true,
        )
        .await
    {
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    print_ocr_results(&results, args.json);

    Ok(())
}

async fn handle_chat(args: ChatArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    // Chat may have its own -v flag (from ChatArgs.verbose).
    // If chat-specific verbosity is higher than the global one, upgrade it.
    // Chat ignores quiet mode (interactive — the user is watching the screen).
    if args.verbose > 0 {
        let chat_verbosity = crate::logging::Verbosity::resolve(false, args.verbose, None);
        crate::logging::set_verbosity(chat_verbosity);
    }

    let db_path: Option<std::path::PathBuf> = cli.db.as_ref().map(std::path::PathBuf::from);

    chat::run_chat_repl(
        settings,
        &args,
        cli.model.as_deref(),
        cli.think,
        cli.tools,
        cli.code,
        cli.ignore_agents,
        args.soulless, // Use chat-specific flag, not global CLI flag
        db_path,
    )
    .await
}

async fn handle_summarize(args: SummarizeArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    let (subcommand_model, _, _) = settings.get_subcommand_config("summarize");

    let model_id = if let Some(ref m) = cli.model {
        m.clone()
    } else if !subcommand_model.is_empty() {
        subcommand_model
    } else {
        settings.model.default.clone()
    };

    let output_flags = OutputFlags::resolve(cli.plain);

    log::debug!("Debug Mode - Summarize Configuration:");
    log::debug!("==========================");
    log::debug!("Model ID:          {}", model_id);
    log::debug!("Max Length:        {} words", args.max_length);
    log::debug!("Format:            {:?}", args.format);
    log::debug!("Style:             {:?}", args.style);
    log::debug!("Plain Output:      {}", output_flags.plain);
    log::debug!("==========================");
    log::debug!("Executing summarization with logging enabled...");

    let text = if let Some(ref text) = args.text {
        text.clone()
    } else {
        match crate::utils::read_stdin() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("Usage: ask summarize [OPTIONS] <TEXT>");
                eprintln!("   or: echo \"text\" | ask summarize");
                std::process::exit(1);
            }
        }
    };

    let processor = SummarizeProcessor::new();

    match processor.summarize(&args, &text, &model_id, settings).await {
        Ok(summary) => {
            if output_flags.plain {
                println!("{}", summary);
            } else {
                markdown::print_markdown(&summary);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_completion(args: CompletionArgs, _settings: &Settings) -> AppResult<()> {
    use clap::CommandFactory;
    use std::io::stdout;

    let cmd = Cli::command();
    let name = cmd.get_name().to_string();

    match args.shell {
        Shell::Bash => clap_complete::generate(
            clap_complete::Shell::Bash,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::Zsh => clap_complete::generate(
            clap_complete::Shell::Zsh,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::Fish => clap_complete::generate(
            clap_complete::Shell::Fish,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::PowerShell => clap_complete::generate(
            clap_complete::Shell::PowerShell,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::Elvish => clap_complete::generate(
            clap_complete::Shell::Elvish,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
    }

    Ok(())
}

async fn handle_vision(args: VisionArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Security: validate all file paths against blocklist + CWD sandbox
    let expanded_paths: Vec<std::path::PathBuf> = args
        .files
        .iter()
        .map(|p| crate::utils::expand_tilde_path(&p.to_string_lossy()))
        .collect();
    if let Err(e) = crate::security::validate_subagent_paths(&expanded_paths) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let (subcommand_model, _, _) = settings.get_subcommand_config("vision");

    let model_name = if let Some(ref m) = cli.model {
        m.clone()
    } else if let Some(ref m) = args.model {
        m.clone()
    } else if !subcommand_model.is_empty() {
        subcommand_model
    } else {
        settings.model.default.clone()
    };

    let model_config = user_models::resolve_model_config(&model_name);
    let model_id = model_config.model_id.clone();

    let output_flags = OutputFlags::resolve(cli.plain);

    log::debug!("Debug Mode - Vision Configuration:");
    log::debug!("==========================");
    log::debug!("Model:             {}", model_id);
    log::debug!("Files:             {:?}", args.files);
    log::debug!("Prompt:            {}", args.get_prompt());
    log::debug!("Detailed:          {}", args.detailed);
    log::debug!("JSON Output:       {}", args.json);
    log::debug!("Max Tokens:        {}", args.max_tokens);
    log::debug!("==========================");
    log::debug!("Executing vision analysis with logging enabled...");

    let model_options = model_config
        .build_model_options()
        .num_predict(args.max_tokens as i32);
    let ollama = settings.ollama_client();
    let processor = VisionProcessor::new();

    match processor
        .process(&args, &model_id, &ollama, model_options, true)
        .await
    {
        Ok(result) => {
            if args.json {
                print_vision_results(&result, true);
            } else if output_flags.plain {
                println!("{}", result.content);
            } else {
                markdown::print_markdown(&result.content);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_thinking_tags() {
        let input = "<think>This is thinking</think>Response.";
        let expected = "Response.";
        assert_eq!(chat::strip_thinking_tags(input), expected);

        let input_no_think = "Just a normal response.";
        assert_eq!(
            chat::strip_thinking_tags(input_no_think),
            "Just a normal response."
        );

        let input_multiline = "<think>\nThinking...\n</think>\n\nFinal answer.";
        let expected_multiline = "Final answer.";
        assert_eq!(
            chat::strip_thinking_tags(input_multiline),
            expected_multiline
        );

        let input_upper = "<THINK>Thinking...</THINK>Response.";
        let expected_upper = "Response.";
        assert_eq!(chat::strip_thinking_tags(input_upper), expected_upper);

        let input_multi = "<think>First</think>Part 1. <think>Second</think>Part 2.";
        let expected_multi = "Part 1. Part 2.";
        assert_eq!(chat::strip_thinking_tags(input_multi), expected_multi);
    }
}
