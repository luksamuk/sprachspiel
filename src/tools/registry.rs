//! Tool registration
//!
//! Centralized tool registration for use across query, legacy query, and chat modes.
//! Handles feature flags and blacklist filtering.

use crate::settings::Settings;

#[cfg(any(
    feature = "pokemon-tools",
    feature = "weather-tools",
    feature = "calc-tools",
    feature = "serper-tools",
    feature = "search-tools",
    feature = "system-tools",
    feature = "file-tools",
    feature = "todo-tools"
))]
use super::*;

// Remember tool is always available (checks context internally)
use super::remember;

// Fact tools (always available)
use super::fact_tools::{fact_add, fact_remove, fact_search};

// Notes tools (always available)
use super::notes::note_add;

// Document import tool
#[cfg(feature = "document-tools")]
use super::documents::import_document;

// External tool wrappers (always available)
use super::{check_tool_availability, run_command};

// Skills tools (on-demand skill loading)
#[cfg(feature = "skills-tools")]
use super::skill_tools::{skill_list, skill_view};

/// Trait for tool registration - implemented by both Coordinator types
pub trait ToolRegistrar: Sized {
    fn register_tool<T: ollama_rs::generation::tools::Tool + 'static>(self, tool: T) -> Self;
}

impl<C: ollama_rs::history::ChatHistory> ToolRegistrar for ollama_rs::coordinator::Coordinator<C> {
    fn register_tool<T: ollama_rs::generation::tools::Tool + 'static>(self, tool: T) -> Self {
        self.add_tool(tool)
    }
}

impl<C: ollama_rs::history::ChatHistory> ToolRegistrar
    for crate::chat::custom_coordinator::CustomCoordinator<C>
{
    fn register_tool<T: ollama_rs::generation::tools::Tool + 'static>(self, tool: T) -> Self {
        self.add_tool(tool)
    }
}

/// Register all available tools with any coordinator implementing ToolRegistrar
///
/// Returns the updated coordinator and the number of tools registered.
/// Tools are filtered based on:
/// - Feature flags (compile-time)
/// - Settings blacklist (runtime)
/// - API key availability (for Serper)
pub fn register_tools<C>(mut coordinator: C, settings: &Settings, use_debug: bool) -> (C, usize)
where
    C: ToolRegistrar,
{
    let is_tool_allowed = |name: &str| !settings.is_tool_blacklisted(name);
    let mut tool_count = 0;

    // Always register test_tool
    coordinator = coordinator.register_tool(test_tool);
    tool_count += 1;

    // Pokemon tools
    #[cfg(feature = "pokemon-tools")]
    {
        if is_tool_allowed("fetch_pokemon") {
            coordinator = coordinator.register_tool(fetch_pokemon);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_basic") {
            coordinator = coordinator.register_tool(fetch_pokemon_basic);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_stats") {
            coordinator = coordinator.register_tool(fetch_pokemon_stats);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_moves") {
            coordinator = coordinator.register_tool(fetch_pokemon_moves);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_evolution") {
            coordinator = coordinator.register_tool(fetch_pokemon_evolution);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_ability_details") {
            coordinator = coordinator.register_tool(fetch_ability_details);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_type_effectiveness") {
            coordinator = coordinator.register_tool(fetch_type_effectiveness);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_by_type") {
            coordinator = coordinator.register_tool(fetch_pokemon_by_type);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_move_details") {
            coordinator = coordinator.register_tool(fetch_move_details);
            tool_count += 1;
        }
    }

    // Weather tools
    #[cfg(feature = "weather-tools")]
    {
        if is_tool_allowed("get_weather") {
            coordinator = coordinator.register_tool(get_weather);
            tool_count += 1;
        }
        if is_tool_allowed("get_current_weather") {
            coordinator = coordinator.register_tool(get_current_weather);
            tool_count += 1;
        }
        if is_tool_allowed("get_weather_forecast") {
            coordinator = coordinator.register_tool(get_weather_forecast);
            tool_count += 1;
        }
    }

    // Calculator tool
    #[cfg(feature = "calc-tools")]
    {
        if is_tool_allowed("calculate") {
            coordinator = coordinator.register_tool(calculate);
            tool_count += 1;
        }
    }

    // Web search tools - prefer Serper over DDG
    #[cfg(feature = "serper-tools")]
    {
        if super::serper::is_serper_available() {
            if use_debug {
                eprintln!("🔑 [Serper] API key found - enabling Google Search via Serper");
            }
            if is_tool_allowed("web_search") {
                coordinator = coordinator.register_tool(super::serper::web_search);
                tool_count += 1;
            }
            if is_tool_allowed("web_search_news") {
                coordinator = coordinator.register_tool(super::serper::web_search_news);
                tool_count += 1;
            }
        } else {
            #[cfg(feature = "search-tools")]
            {
                if use_debug {
                    eprintln!(
                        "ℹ️  [Search] SERPER_API_KEY not set - using DuckDuckGo (may be blocked by CAPTCHA)"
                    );
                }
                if is_tool_allowed("web_search") {
                    coordinator = coordinator.register_tool(web_search);
                    tool_count += 1;
                }
                if is_tool_allowed("web_search_news") {
                    coordinator = coordinator.register_tool(web_search_news);
                    tool_count += 1;
                }
            }
            #[cfg(not(feature = "search-tools"))]
            {
                if use_debug {
                    eprintln!(
                        "⚠️  [Search] No search available - set SERPER_API_KEY or enable search-tools feature"
                    );
                }
            }
        }
    }

    // DDG fallback when serper-tools not enabled but search-tools is
    #[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
    {
        if is_tool_allowed("web_search") {
            coordinator = coordinator.register_tool(web_search);
            tool_count += 1;
        }
        if is_tool_allowed("web_search_news") {
            coordinator = coordinator.register_tool(web_search_news);
            tool_count += 1;
        }
    }

    // Web scraper (always with search-tools)
    #[cfg(feature = "search-tools")]
    {
        if is_tool_allowed("web_scrape") {
            coordinator = coordinator.register_tool(web_scrape);
            tool_count += 1;
        }
    }

    // Finance tools
    #[cfg(feature = "finance-tools")]
    {
        if is_tool_allowed("get_stock_quote") {
            coordinator = coordinator.register_tool(get_stock_quote);
            tool_count += 1;
        }
    }

    // System tools
    #[cfg(feature = "system-tools")]
    {
        if is_tool_allowed("get_current_datetime") {
            coordinator = coordinator.register_tool(get_current_datetime);
            tool_count += 1;
        }
        if is_tool_allowed("get_project_context") {
            coordinator = coordinator.register_tool(get_project_context);
            tool_count += 1;
        }
    }

    // File tools
    #[cfg(feature = "file-tools")]
    {
        if is_tool_allowed("read_file") {
            coordinator = coordinator.register_tool(read_file);
            tool_count += 1;
        }
        if is_tool_allowed("read_file_segment") {
            coordinator = coordinator.register_tool(read_file_segment);
            tool_count += 1;
        }
        if is_tool_allowed("count_lines") {
            coordinator = coordinator.register_tool(count_lines);
            tool_count += 1;
        }
        if is_tool_allowed("list_directory") {
            coordinator = coordinator.register_tool(list_directory);
            tool_count += 1;
        }
        if is_tool_allowed("search_files") {
            coordinator = coordinator.register_tool(search_files);
            tool_count += 1;
        }
        // File write tools
        if is_tool_allowed("write_file") {
            coordinator = coordinator.register_tool(write_file);
            tool_count += 1;
        }
        if is_tool_allowed("edit_file") {
            coordinator = coordinator.register_tool(edit_file);
            tool_count += 1;
        }
        if is_tool_allowed("append_file") {
            coordinator = coordinator.register_tool(append_file);
            tool_count += 1;
        }
    }

    // LED tools (requires configuration)
    #[cfg(feature = "led-tools")]
    {
        if settings.is_led_configured() {
            // Initialize the LED endpoint from settings
            super::led::set_led_endpoint(settings.led_endpoint());

            if use_debug {
                eprintln!(
                    "💡 [LED] Device configured at {}",
                    settings.led_endpoint().unwrap_or_default()
                );
            }

            if is_tool_allowed("led_get_status") {
                coordinator = coordinator.register_tool(led_get_status);
                tool_count += 1;
            }
            if is_tool_allowed("led_set_power") {
                coordinator = coordinator.register_tool(led_set_power);
                tool_count += 1;
            }
            if is_tool_allowed("led_set_program") {
                coordinator = coordinator.register_tool(led_set_program);
                tool_count += 1;
            }
            if is_tool_allowed("led_set_brightness") {
                coordinator = coordinator.register_tool(led_set_brightness);
                tool_count += 1;
            }
            if is_tool_allowed("led_set_color") {
                coordinator = coordinator.register_tool(led_set_color);
                tool_count += 1;
            }
        } else if use_debug {
            eprintln!(
                "💡 [LED] No device configured - LED tools disabled. Add [led] ip = \"<IP>\" to config.toml"
            );
        }
    }

    // Todo tools
    #[cfg(feature = "todo-tools")]
    {
        if is_tool_allowed("todo_add") {
            coordinator = coordinator.register_tool(todo_add);
            tool_count += 1;
        }
        if is_tool_allowed("todo_update") {
            coordinator = coordinator.register_tool(todo_update);
            tool_count += 1;
        }
        if is_tool_allowed("todo_list") {
            coordinator = coordinator.register_tool(todo_list);
            tool_count += 1;
        }
        if is_tool_allowed("todo_clear_done") {
            coordinator = coordinator.register_tool(todo_clear_done);
            tool_count += 1;
        }
        if is_tool_allowed("todo_clear_all") {
            coordinator = coordinator.register_tool(todo_clear_all);
            tool_count += 1;
        }
    }

    // Remember tool - always available (checks context internally)
    // Note: The tool returns an error if DB/EmbeddingClient not available
    if is_tool_allowed("remember") {
        coordinator = coordinator.register_tool(remember);
        tool_count += 1;
    }

    // Fact tools - always available (checks context internally)
    if is_tool_allowed("fact_add") {
        coordinator = coordinator.register_tool(fact_add);
        tool_count += 1;
    }
    if is_tool_allowed("fact_search") {
        coordinator = coordinator.register_tool(fact_search);
        tool_count += 1;
    }
    if is_tool_allowed("fact_remove") {
        coordinator = coordinator.register_tool(fact_remove);
        tool_count += 1;
    }

    // Notes tools - always available (checks context internally)
    if is_tool_allowed("note_add") {
        coordinator = coordinator.register_tool(note_add);
        tool_count += 1;
    }

    // Document import tool
    #[cfg(feature = "document-tools")]
    {
        if is_tool_allowed("import_document") {
            coordinator = coordinator.register_tool(import_document);
            tool_count += 1;
        }
    }

    // External tool wrappers (always available)
    // These tools check for external CLI tools like pdftotext, tesseract, etc.
    if is_tool_allowed("check_tool_availability") {
        coordinator = coordinator.register_tool(check_tool_availability);
        tool_count += 1;
    }
    if is_tool_allowed("run_command") {
        coordinator = coordinator.register_tool(run_command);
        tool_count += 1;
    }

    // Skills tools (on-demand skill loading)
    #[cfg(feature = "skills-tools")]
    {
        if is_tool_allowed("skill_list") {
            coordinator = coordinator.register_tool(skill_list);
            tool_count += 1;
        }
        if is_tool_allowed("skill_view") {
            coordinator = coordinator.register_tool(skill_view);
            tool_count += 1;
        }
    }

    (coordinator, tool_count)
}

/// Get list of available tool names (for logging/error messages)
pub fn get_available_tool_names(settings: &Settings) -> Vec<String> {
    let mut tools = Vec::new();
    let is_allowed = |name: &str| !settings.is_tool_blacklisted(name);

    tools.push("test_tool".to_string());
    // Remember tool - always available (checks context internally)
    tools.push("remember".to_string());

    // Fact tools - always available (checks context internally)
    tools.push("fact_add".to_string());
    tools.push("fact_search".to_string());
    tools.push("fact_remove".to_string());

    // Notes tools - always available (checks context internally)
    tools.push("note_add".to_string());

    // Document import tool
    #[cfg(feature = "document-tools")]
    {
        if is_allowed("import_document") {
            tools.push("import_document".to_string());
        }
    }

    // External tool wrappers (always available)
    tools.push("check_tool_availability".to_string());
    tools.push("run_command".to_string());

    // Skills tools (on-demand skill loading)
    #[cfg(feature = "skills-tools")]
    {
        tools.push("skill_list".to_string());
        tools.push("skill_view".to_string());
    }

    #[cfg(feature = "pokemon-tools")]
    {
        let pokemon_tools = [
            "fetch_pokemon",
            "fetch_pokemon_basic",
            "fetch_pokemon_stats",
            "fetch_pokemon_moves",
            "fetch_pokemon_evolution",
            "fetch_ability_details",
            "fetch_type_effectiveness",
            "fetch_pokemon_by_type",
            "fetch_move_details",
        ];
        for tool in pokemon_tools {
            if is_allowed(tool) {
                tools.push(tool.to_string());
            }
        }
    }

    #[cfg(feature = "weather-tools")]
    {
        let weather_tools = ["get_weather", "get_current_weather", "get_weather_forecast"];
        for tool in weather_tools {
            if is_allowed(tool) {
                tools.push(tool.to_string());
            }
        }
    }

    #[cfg(feature = "calc-tools")]
    {
        if is_allowed("calculate") {
            tools.push("calculate".to_string());
        }
    }

    #[cfg(feature = "serper-tools")]
    {
        if super::serper::is_serper_available() {
            if is_allowed("web_search") {
                tools.push("web_search".to_string());
            }
            if is_allowed("web_search_news") {
                tools.push("web_search_news".to_string());
            }
        }
    }

    #[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
    {
        let search_tools = ["web_search", "web_search_news", "web_scrape"];
        for tool in search_tools {
            if is_allowed(tool) {
                tools.push(tool.to_string());
            }
        }
    }

    #[cfg(feature = "system-tools")]
    {
        let system_tools = ["get_current_datetime", "get_project_context"];
        for tool in system_tools {
            if is_allowed(tool) {
                tools.push(tool.to_string());
            }
        }
    }

    #[cfg(feature = "file-tools")]
    {
        let file_tools = [
            "read_file",
            "read_file_segment",
            "count_lines",
            "list_directory",
            "search_files",
        ];
        for tool in file_tools {
            if is_allowed(tool) {
                tools.push(tool.to_string());
            }
        }
    }

    #[cfg(feature = "led-tools")]
    {
        if settings.is_led_configured() {
            let led_tools = [
                "led_get_status",
                "led_set_power",
                "led_set_program",
                "led_set_brightness",
                "led_set_color",
            ];
            for tool in led_tools {
                if is_allowed(tool) {
                    tools.push(tool.to_string());
                }
            }
        }
    }

    #[cfg(feature = "todo-tools")]
    {
        let todo_tools = [
            "todo_add",
            "todo_update",
            "todo_list",
            "todo_clear_done",
            "todo_clear_all",
        ];
        for tool in todo_tools {
            if is_allowed(tool) {
                tools.push(tool.to_string());
            }
        }
    }

    tools
}
