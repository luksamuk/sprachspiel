//! Tool registration
//!
//! Centralized tool registration for use across query, legacy query, and chat modes.
//! Handles feature flags and blacklist filtering.

use ollama_rs::coordinator::Coordinator;
use ollama_rs::history::ChatHistory;

use crate::settings::Settings;

#[cfg(any(
    feature = "pokemon-tools",
    feature = "weather-tools",
    feature = "calc-tools",
    feature = "serper-tools",
    feature = "search-tools",
    feature = "system-tools",
    feature = "file-tools"
))]
use super::*;

/// Register all available tools with the coordinator
///
/// Returns the updated coordinator and the number of tools registered.
/// Tools are filtered based on:
/// - Feature flags (compile-time)
/// - Settings blacklist (runtime)
/// - API key availability (for Serper)
pub fn register_tools<C>(
    mut coordinator: Coordinator<C>,
    settings: &Settings,
    use_debug: bool,
) -> (Coordinator<C>, usize)
where
    C: ChatHistory + Clone,
{
    let is_tool_allowed = |name: &str| !settings.is_tool_blacklisted(name);
    let mut tool_count = 0;

    // Always register test_tool
    coordinator = coordinator.add_tool(test_tool);
    tool_count += 1;

    // Pokemon tools
    #[cfg(feature = "pokemon-tools")]
    {
        if is_tool_allowed("fetch_pokemon") {
            coordinator = coordinator.add_tool(fetch_pokemon);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_basic") {
            coordinator = coordinator.add_tool(fetch_pokemon_basic);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_stats") {
            coordinator = coordinator.add_tool(fetch_pokemon_stats);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_moves") {
            coordinator = coordinator.add_tool(fetch_pokemon_moves);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_evolution") {
            coordinator = coordinator.add_tool(fetch_pokemon_evolution);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_ability_details") {
            coordinator = coordinator.add_tool(fetch_ability_details);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_type_effectiveness") {
            coordinator = coordinator.add_tool(fetch_type_effectiveness);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_pokemon_by_type") {
            coordinator = coordinator.add_tool(fetch_pokemon_by_type);
            tool_count += 1;
        }
        if is_tool_allowed("fetch_move_details") {
            coordinator = coordinator.add_tool(fetch_move_details);
            tool_count += 1;
        }
    }

    // Weather tools
    #[cfg(feature = "weather-tools")]
    {
        if is_tool_allowed("get_weather") {
            coordinator = coordinator.add_tool(get_weather);
            tool_count += 1;
        }
        if is_tool_allowed("get_current_weather") {
            coordinator = coordinator.add_tool(get_current_weather);
            tool_count += 1;
        }
        if is_tool_allowed("get_weather_forecast") {
            coordinator = coordinator.add_tool(get_weather_forecast);
            tool_count += 1;
        }
    }

    // Calculator tool
    #[cfg(feature = "calc-tools")]
    {
        if is_tool_allowed("calculate") {
            coordinator = coordinator.add_tool(calculate);
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
                coordinator = coordinator.add_tool(super::serper::web_search);
                tool_count += 1;
            }
            if is_tool_allowed("web_search_news") {
                coordinator = coordinator.add_tool(super::serper::web_search_news);
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
                    coordinator = coordinator.add_tool(web_search);
                    tool_count += 1;
                }
                if is_tool_allowed("web_search_news") {
                    coordinator = coordinator.add_tool(web_search_news);
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
            coordinator = coordinator.add_tool(web_search);
            tool_count += 1;
        }
        if is_tool_allowed("web_search_news") {
            coordinator = coordinator.add_tool(web_search_news);
            tool_count += 1;
        }
    }

    // Web scraper (always with search-tools)
    #[cfg(feature = "search-tools")]
    {
        if is_tool_allowed("web_scrape") {
            coordinator = coordinator.add_tool(web_scrape);
            tool_count += 1;
        }
    }

    // Finance tools
    #[cfg(feature = "finance-tools")]
    {
        if is_tool_allowed("get_stock_quote") {
            coordinator = coordinator.add_tool(get_stock_quote);
            tool_count += 1;
        }
    }

    // System tools
    #[cfg(feature = "system-tools")]
    {
        if is_tool_allowed("get_current_datetime") {
            coordinator = coordinator.add_tool(get_current_datetime);
            tool_count += 1;
        }
        if is_tool_allowed("get_project_context") {
            coordinator = coordinator.add_tool(get_project_context);
            tool_count += 1;
        }
    }

    // File tools
    #[cfg(feature = "file-tools")]
    {
        if is_tool_allowed("read_file") {
            coordinator = coordinator.add_tool(read_file);
            tool_count += 1;
        }
        if is_tool_allowed("read_file_segment") {
            coordinator = coordinator.add_tool(read_file_segment);
            tool_count += 1;
        }
        if is_tool_allowed("count_lines") {
            coordinator = coordinator.add_tool(count_lines);
            tool_count += 1;
        }
        if is_tool_allowed("list_directory") {
            coordinator = coordinator.add_tool(list_directory);
            tool_count += 1;
        }
        if is_tool_allowed("search_files") {
            coordinator = coordinator.add_tool(search_files);
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

    tools
}
