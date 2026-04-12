//! Tool registration
//!
//! Centralized tool registration for use across query, legacy query, and chat modes.
//! Handles feature flags and blacklist filtering.

use crate::log_if_debug;
use crate::settings::Settings;

#[cfg(any(
    feature = "pokemon-tools",
    feature = "weather-tools",
    feature = "calc-tools",
    feature = "serper-tools",
    feature = "search-tools",
    feature = "system-tools",
    feature = "file-tools",
    feature = "finance-tools"
))]
use super::*;

// Remember tool is always available (checks context internally)
use super::remember;

// Fact tools (always available)
use super::fact_tools::{fact_add, fact_remove, fact_search};

// Notes tools (always available)
use super::notes::{note_add, note_delete, note_edit};

// Todo tools (always available)
use super::todo::{
    todo_add, todo_clear_all, todo_clear_done, todo_delete, todo_edit, todo_get, todo_list,
    todo_update,
};

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

// =============================================================================
// Tool Registration Helpers (by category)
// =============================================================================

/// Helper macro to register a tool if allowed
macro_rules! register_if_allowed {
    ($coordinator:expr, $count:expr, $is_allowed:expr, $tool_name:expr, $tool:expr) => {
        if $is_allowed($tool_name) {
            $coordinator = $coordinator.register_tool($tool);
            $count += 1;
        }
    };
}

/// Register core tools (always available, checks context internally)
fn register_core_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    // test_tool - debug tool for testing tool calling
    register_if_allowed!(coord, count, is_allowed, "test_tool", test_tool);

    // remember - always available (checks context internally)
    register_if_allowed!(coord, count, is_allowed, "remember", remember);

    // fact tools - always available (checks context internally)
    register_if_allowed!(coord, count, is_allowed, "fact_add", fact_add);
    register_if_allowed!(coord, count, is_allowed, "fact_search", fact_search);
    register_if_allowed!(coord, count, is_allowed, "fact_remove", fact_remove);

    // notes tools - always available (checks context internally)
    register_if_allowed!(coord, count, is_allowed, "note_add", note_add);
    register_if_allowed!(coord, count, is_allowed, "note_edit", note_edit);
    register_if_allowed!(coord, count, is_allowed, "note_delete", note_delete);

    // external tool wrappers (always available)
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "check_tool_availability",
        check_tool_availability
    );
    register_if_allowed!(coord, count, is_allowed, "run_command", run_command);

    (coord, count)
}

/// Register document tools
#[cfg(feature = "document-tools")]
fn register_document_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "import_document", import_document);

    (coord, count)
}

/// Register skills tools
#[cfg(feature = "skills-tools")]
fn register_skills_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "skill_list", skill_list);
    register_if_allowed!(coord, count, is_allowed, "skill_view", skill_view);

    (coord, count)
}

/// Register Pokemon tools
#[cfg(feature = "pokemon-tools")]
fn register_pokemon_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "fetch_pokemon", fetch_pokemon);
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_pokemon_basic",
        fetch_pokemon_basic
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_pokemon_stats",
        fetch_pokemon_stats
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_pokemon_moves",
        fetch_pokemon_moves
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_pokemon_evolution",
        fetch_pokemon_evolution
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_ability_details",
        fetch_ability_details
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_type_effectiveness",
        fetch_type_effectiveness
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_pokemon_by_type",
        fetch_pokemon_by_type
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "fetch_move_details",
        fetch_move_details
    );

    (coord, count)
}

/// Register weather tools
#[cfg(feature = "weather-tools")]
fn register_weather_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "get_weather", get_weather);
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "get_current_weather",
        get_current_weather
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "get_weather_forecast",
        get_weather_forecast
    );

    (coord, count)
}

/// Register calculator tool
#[cfg(feature = "calc-tools")]
fn register_calc_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "calculate", calculate);

    (coord, count)
}

/// Register finance tools
#[cfg(feature = "finance-tools")]
fn register_finance_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "get_stock_quote", get_stock_quote);

    (coord, count)
}

/// Register search tools (Serper API preferred)
#[cfg(feature = "serper-tools")]
fn register_search_tools_serper<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
    use_debug: bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    if super::serper::is_serper_available() {
        log_if_debug!(
            use_debug,
            "🔑 [Serper] API key found - enabling Google Search via Serper"
        );
        register_if_allowed!(
            coord,
            count,
            is_allowed,
            "web_search",
            super::serper::web_search
        );
        register_if_allowed!(
            coord,
            count,
            is_allowed,
            "web_search_news",
            super::serper::web_search_news
        );
        // web_scrape is always available with search-tools, even when using Serper
        #[cfg(feature = "search-tools")]
        register_if_allowed!(coord, count, is_allowed, "web_scrape", web_scrape);
    } else {
        #[cfg(feature = "search-tools")]
        {
            log_if_debug!(
                use_debug,
                "ℹ️  [Search] SERPER_API_KEY not set - using DuckDuckGo (may be blocked by CAPTCHA)"
            );
            register_if_allowed!(coord, count, is_allowed, "web_search", web_search);
            register_if_allowed!(coord, count, is_allowed, "web_search_news", web_search_news);
            register_if_allowed!(coord, count, is_allowed, "web_scrape", web_scrape);
        }
        #[cfg(not(feature = "search-tools"))]
        {
            log_if_debug!(
                use_debug,
                "⚠️  [Search] No search available - set SERPER_API_KEY or enable search-tools feature"
            );
        }
    }

    (coord, count)
}

/// Register search tools (DuckDuckGo fallback when Serper not enabled)
#[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
fn register_search_tools_ddg<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "web_search", web_search);
    register_if_allowed!(coord, count, is_allowed, "web_search_news", web_search_news);
    register_if_allowed!(coord, count, is_allowed, "web_scrape", web_scrape);

    (coord, count)
}

/// Register system tools
#[cfg(feature = "system-tools")]
fn register_system_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "get_current_datetime",
        get_current_datetime
    );
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "get_project_context",
        get_project_context
    );

    (coord, count)
}

/// Register file tools
#[cfg(feature = "file-tools")]
fn register_file_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    // file read tools
    register_if_allowed!(coord, count, is_allowed, "read_file", read_file);
    register_if_allowed!(
        coord,
        count,
        is_allowed,
        "read_file_segment",
        read_file_segment
    );
    register_if_allowed!(coord, count, is_allowed, "count_lines", count_lines);
    register_if_allowed!(coord, count, is_allowed, "list_directory", list_directory);
    register_if_allowed!(coord, count, is_allowed, "search_files", search_files);

    // file write tools
    register_if_allowed!(coord, count, is_allowed, "write_file", write_file);
    register_if_allowed!(coord, count, is_allowed, "edit_file", edit_file);
    register_if_allowed!(coord, count, is_allowed, "append_file", append_file);

    (coord, count)
}

/// Register LED tools (requires configuration)
#[cfg(feature = "led-tools")]
fn register_led_tools<C: ToolRegistrar>(
    coordinator: C,
    settings: &Settings,
    is_allowed: impl Fn(&str) -> bool,
    use_debug: bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    if settings.is_led_configured() {
        // Initialize the LED endpoint from settings
        super::led::set_led_endpoint(settings.led_endpoint());

        log_if_debug!(
            use_debug,
            "💡 [LED] Device configured at {}",
            settings.led_endpoint().unwrap_or_default()
        );

        register_if_allowed!(coord, count, is_allowed, "led_get_status", led_get_status);
        register_if_allowed!(coord, count, is_allowed, "led_set_power", led_set_power);
        register_if_allowed!(coord, count, is_allowed, "led_set_program", led_set_program);
        register_if_allowed!(
            coord,
            count,
            is_allowed,
            "led_set_brightness",
            led_set_brightness
        );
        register_if_allowed!(coord, count, is_allowed, "led_set_color", led_set_color);
    } else {
        log_if_debug!(
            use_debug,
            "💡 [LED] No device configured - LED tools disabled. Add [led] ip = \"<IP>\" to config.toml"
        );
    }

    (coord, count)
}

/// Register todo tools (always available)
fn register_todo_tools<C: ToolRegistrar>(
    coordinator: C,
    is_allowed: impl Fn(&str) -> bool,
) -> (C, usize) {
    let mut count = 0;
    let mut coord = coordinator;

    register_if_allowed!(coord, count, is_allowed, "todo_add", todo_add);
    register_if_allowed!(coord, count, is_allowed, "todo_update", todo_update);
    register_if_allowed!(coord, count, is_allowed, "todo_get", todo_get);
    register_if_allowed!(coord, count, is_allowed, "todo_edit", todo_edit);
    register_if_allowed!(coord, count, is_allowed, "todo_delete", todo_delete);
    register_if_allowed!(coord, count, is_allowed, "todo_list", todo_list);
    register_if_allowed!(coord, count, is_allowed, "todo_clear_done", todo_clear_done);
    register_if_allowed!(coord, count, is_allowed, "todo_clear_all", todo_clear_all);

    (coord, count)
}

// =============================================================================
// Tool Name Listing Helpers (by category)
// =============================================================================

/// Helper macro to add a tool name if allowed
macro_rules! push_if_allowed {
    ($tools:expr, $is_allowed:expr, $tool_name:expr) => {
        if $is_allowed($tool_name) {
            $tools.push($tool_name.to_string());
        }
    };
}

/// Get core tool names (always available)
fn get_core_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();

    push_if_allowed!(tools, is_allowed, "test_tool");
    push_if_allowed!(tools, is_allowed, "remember");
    push_if_allowed!(tools, is_allowed, "fact_add");
    push_if_allowed!(tools, is_allowed, "fact_search");
    push_if_allowed!(tools, is_allowed, "fact_remove");
    push_if_allowed!(tools, is_allowed, "note_add");
    push_if_allowed!(tools, is_allowed, "note_edit");
    push_if_allowed!(tools, is_allowed, "note_delete");
    push_if_allowed!(tools, is_allowed, "check_tool_availability");
    push_if_allowed!(tools, is_allowed, "run_command");

    tools
}

/// Get document tool names
#[cfg(feature = "document-tools")]
fn get_document_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    push_if_allowed!(tools, is_allowed, "import_document");
    tools
}

/// Get skills tool names
#[cfg(feature = "skills-tools")]
fn get_skills_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    push_if_allowed!(tools, is_allowed, "skill_list");
    push_if_allowed!(tools, is_allowed, "skill_view");
    tools
}

/// Get Pokemon tool names
#[cfg(feature = "pokemon-tools")]
fn get_pokemon_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
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
        push_if_allowed!(tools, is_allowed, tool);
    }
    tools
}

/// Get weather tool names
#[cfg(feature = "weather-tools")]
fn get_weather_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    let weather_tools = ["get_weather", "get_current_weather", "get_weather_forecast"];
    for tool in weather_tools {
        push_if_allowed!(tools, is_allowed, tool);
    }
    tools
}

/// Get calculator tool names
#[cfg(feature = "calc-tools")]
fn get_calc_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    push_if_allowed!(tools, is_allowed, "calculate");
    tools
}

/// Get finance tool names
#[cfg(feature = "finance-tools")]
fn get_finance_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    push_if_allowed!(tools, is_allowed, "get_stock_quote");
    tools
}

/// Get search tool names (Serper API)
#[cfg(feature = "serper-tools")]
fn get_search_tool_names_serper(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();

    if super::serper::is_serper_available() {
        push_if_allowed!(tools, is_allowed, "web_search");
        push_if_allowed!(tools, is_allowed, "web_search_news");
        // web_scrape is always available with search-tools, even when using Serper
        #[cfg(feature = "search-tools")]
        push_if_allowed!(tools, is_allowed, "web_scrape");
    } else {
        #[cfg(feature = "search-tools")]
        {
            push_if_allowed!(tools, is_allowed, "web_search");
            push_if_allowed!(tools, is_allowed, "web_search_news");
            push_if_allowed!(tools, is_allowed, "web_scrape");
        }
    }

    tools
}

/// Get search tool names (DuckDuckGo fallback)
#[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
fn get_search_tool_names_ddg(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    push_if_allowed!(tools, is_allowed, "web_search");
    push_if_allowed!(tools, is_allowed, "web_search_news");
    push_if_allowed!(tools, is_allowed, "web_scrape");
    tools
}

/// Get system tool names
#[cfg(feature = "system-tools")]
fn get_system_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    let system_tools = ["get_current_datetime", "get_project_context"];
    for tool in system_tools {
        push_if_allowed!(tools, is_allowed, tool);
    }
    tools
}

/// Get file tool names
#[cfg(feature = "file-tools")]
fn get_file_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    let file_tools = [
        "read_file",
        "read_file_segment",
        "count_lines",
        "list_directory",
        "search_files",
        "write_file",
        "edit_file",
        "append_file",
    ];
    for tool in file_tools {
        push_if_allowed!(tools, is_allowed, tool);
    }
    tools
}

/// Get LED tool names
#[cfg(feature = "led-tools")]
fn get_led_tool_names(settings: &Settings, is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();

    if settings.is_led_configured() {
        let led_tools = [
            "led_get_status",
            "led_set_power",
            "led_set_program",
            "led_set_brightness",
            "led_set_color",
        ];
        for tool in led_tools {
            push_if_allowed!(tools, is_allowed, tool);
        }
    }

    tools
}

/// Get todo tool names (always available)
fn get_todo_tool_names(is_allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let mut tools = Vec::new();
    let todo_tools = [
        "todo_add",
        "todo_update",
        "todo_get",
        "todo_edit",
        "todo_delete",
        "todo_list",
        "todo_clear_done",
        "todo_clear_all",
    ];
    for tool in todo_tools {
        push_if_allowed!(tools, is_allowed, tool);
    }
    tools
}

// =============================================================================
// Main Registration Functions
// =============================================================================

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
    let is_allowed = |name: &str| !settings.is_tool_blacklisted(name);
    let mut tool_count = 0;

    // Core tools (always available)
    let (c, n) = register_core_tools(coordinator, is_allowed);
    coordinator = c;
    tool_count += n;

    // Document tools
    #[cfg(feature = "document-tools")]
    {
        let (c, n) = register_document_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // Skills tools
    #[cfg(feature = "skills-tools")]
    {
        let (c, n) = register_skills_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // Pokemon tools
    #[cfg(feature = "pokemon-tools")]
    {
        let (c, n) = register_pokemon_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // Weather tools
    #[cfg(feature = "weather-tools")]
    {
        let (c, n) = register_weather_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // Calculator tools
    #[cfg(feature = "calc-tools")]
    {
        let (c, n) = register_calc_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // Finance tools
    #[cfg(feature = "finance-tools")]
    {
        let (c, n) = register_finance_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // Search tools (Serper preferred, with DDG fallback)
    #[cfg(feature = "serper-tools")]
    {
        let (c, n) = register_search_tools_serper(coordinator, is_allowed, use_debug);
        coordinator = c;
        tool_count += n;
    }

    // DDG fallback when serper-tools not enabled
    #[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
    {
        let (c, n) = register_search_tools_ddg(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // System tools
    #[cfg(feature = "system-tools")]
    {
        let (c, n) = register_system_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // File tools
    #[cfg(feature = "file-tools")]
    {
        let (c, n) = register_file_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    // LED tools (requires configuration)
    #[cfg(feature = "led-tools")]
    {
        let (c, n) = register_led_tools(coordinator, settings, is_allowed, use_debug);
        coordinator = c;
        tool_count += n;
    }

    // Todo tools (always available)
    {
        let (c, n) = register_todo_tools(coordinator, is_allowed);
        coordinator = c;
        tool_count += n;
    }

    (coordinator, tool_count)
}

/// Get list of available tool names (for logging/error messages)
pub fn get_available_tool_names(settings: &Settings) -> Vec<String> {
    let is_allowed = |name: &str| !settings.is_tool_blacklisted(name);
    let mut tools = Vec::new();

    // Core tools (always available)
    tools.extend(get_core_tool_names(is_allowed));

    // Document tools
    #[cfg(feature = "document-tools")]
    {
        tools.extend(get_document_tool_names(is_allowed));
    }

    // Skills tools
    #[cfg(feature = "skills-tools")]
    {
        tools.extend(get_skills_tool_names(is_allowed));
    }

    // Pokemon tools
    #[cfg(feature = "pokemon-tools")]
    {
        tools.extend(get_pokemon_tool_names(is_allowed));
    }

    // Weather tools
    #[cfg(feature = "weather-tools")]
    {
        tools.extend(get_weather_tool_names(is_allowed));
    }

    // Calculator tools
    #[cfg(feature = "calc-tools")]
    {
        tools.extend(get_calc_tool_names(is_allowed));
    }

    // Finance tools
    #[cfg(feature = "finance-tools")]
    {
        tools.extend(get_finance_tool_names(is_allowed));
    }

    // Search tools (Serper)
    #[cfg(feature = "serper-tools")]
    {
        tools.extend(get_search_tool_names_serper(is_allowed));
    }

    // Search tools (DDG fallback)
    #[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
    {
        tools.extend(get_search_tool_names_ddg(is_allowed));
    }

    // System tools
    #[cfg(feature = "system-tools")]
    {
        tools.extend(get_system_tool_names(is_allowed));
    }

    // File tools
    #[cfg(feature = "file-tools")]
    {
        tools.extend(get_file_tool_names(is_allowed));
    }

    // LED tools
    #[cfg(feature = "led-tools")]
    {
        tools.extend(get_led_tool_names(settings, is_allowed));
    }

    // Todo tools (always available)
    tools.extend(get_todo_tool_names(is_allowed));

    tools
}
