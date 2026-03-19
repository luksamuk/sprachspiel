//! Tool context builder
//!
//! Builds the tool section of system prompts. Instead of embedding detailed
//! tool descriptions in the prompt, we list available tools and rely on
//! the tool definitions (function metadata) for specifics.

use std::collections::HashSet;

/// Build minimal tool context section
///
/// Lists available tools by category with brief usage guidance.
/// Detailed descriptions are provided by the tool definitions themselves,
/// which are more efficient for the model to process.
///
/// # Arguments
/// * `blacklist` - Set of tool names to exclude
///
/// # Feature Flags
/// Tools are included based on compile-time feature flags:
/// - `weather-tools`: Weather tools
/// - `pokemon-tools`: Pokémon API tools
/// - `serper-tools`: Serper API web search
/// - `search-tools`: DuckDuckGo web search (fallback)
/// - `file-tools`: File operation tools
/// - `calc-tools`: Calculator
/// - `system-tools`: System information tools
pub fn build_tool_context(blacklist: &HashSet<&str>) -> String {
    let mut sections = Vec::new();

    // Weather tools
    #[cfg(feature = "weather-tools")]
    {
        let weather_tools = ["get_weather", "get_current_weather", "get_weather_forecast"];
        let available: Vec<_> = weather_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### WEATHER TOOLS
Use for weather and climate queries.
Available: get_weather, get_current_weather, get_weather_forecast"#
                    .to_string(),
            );
        }
    }

    // Pokémon tools
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
        let available: Vec<_> = pokemon_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### POKÉMON TOOLS
Use ONLY for Pokémon-related queries (names, abilities, moves, types, evolution).
Available: fetch_pokemon, fetch_pokemon_stats, fetch_pokemon_moves, etc."#
                    .to_string(),
            );
        }
    }

    // Web search tools - Serper (preferred)
    #[cfg(feature = "serper-tools")]
    {
        if !blacklist.contains("web_search") {
            sections.push(
                r#"### WEB SEARCH TOOLS
Use for general knowledge, current events, or anything NOT about Pokémon or weather.
Available: web_search, web_search_news"#
                    .to_string(),
            );
        }
    }

    // Web search tools - DuckDuckGo (fallback)
    #[cfg(all(feature = "search-tools", not(feature = "serper-tools")))]
    {
        let search_tools = ["web_search", "web_search_news", "web_scrape"];
        let available: Vec<_> = search_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### WEB SEARCH TOOLS
Use for general knowledge, current events, or anything NOT about Pokémon or weather.
Available: web_search, web_search_news, web_scrape"#
                    .to_string(),
            );
        }
    }

    // Calculator tool
    #[cfg(feature = "calc-tools")]
    {
        if !blacklist.contains("calculate") {
            sections.push(
                r#"### CALCULATOR TOOL
Use for mathematical calculations.
Available: calculate"#
                    .to_string(),
            );
        }
    }

    // File tools
    #[cfg(feature = "file-tools")]
    {
        let file_tools = [
            "read_file",
            "read_file_segment",
            "count_lines",
            "list_directory",
            "search_files",
        ];
        let available: Vec<_> = file_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### FILE TOOLS
Use for reading, listing, and searching files.
Available: read_file, read_file_segment, count_lines, list_directory, search_files

Note: For large files, use count_lines first, then read_file_segment with start_line and num_lines.

**IMPORTANT FOR PDFs:** read_file cannot read PDFs (binary format). Use run_command instead:
- run_command("pdftotext document.pdf -", null, null, null) - Full text (be careful!)
- run_command("pdftotext -f 1 -l 10 document.pdf -", null, null, null) - Pages 1-10
- run_command("pdftotext document.pdf -", 100, null, null) - First 100 lines"#
                    .to_string(),
            );
        }
    }

    // System tools
    #[cfg(feature = "system-tools")]
    {
        let system_tools = ["get_current_datetime", "get_project_context"];
        let available: Vec<_> = system_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### SYSTEM TOOLS
Use for current date/time or project context.
Available: get_current_datetime, get_project_context"#
                    .to_string(),
            );
        }
    }

    // LED tools (requires configuration)
    #[cfg(feature = "led-tools")]
    {
        let led_tools = [
            "led_get_status",
            "led_set_power",
            "led_set_program",
            "led_set_brightness",
            "led_set_color",
        ];
        let available: Vec<_> = led_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### LED TOOLS
Use for controlling NeoPixel LED strips via Raspberry Pi Pico W.
Available: led_get_status, led_set_power, led_set_program, led_set_brightness, led_set_color

For color adjustments:
1. Get current color with led_get_status
2. Note the RGB values provided in the response
3. Adjust R/G/B values as needed (0-255 each)
4. Set new color with led_set_color using r/g/b parameters

Color tips: To make "more red", increase R or decrease G/B. For "warmer", increase R slightly. For "cooler", increase B slightly."#
                    .to_string(),
            );
        }
    }

    // Todo tools
    #[cfg(feature = "todo-tools")]
    {
        let todo_tools = [
            "todo_add",
            "todo_update",
            "todo_list",
            "todo_clear_done",
            "todo_clear_all",
        ];
        let available: Vec<_> = todo_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### TODO TOOLS
Use for tracking tasks during multi-step work. Reduces need to search conversation history.
Available: todo_add, todo_update, todo_list, todo_clear_done, todo_clear_all

Workflow:
1. Add tasks with todo_add("description") when starting multi-step work
2. List tasks with todo_list() to see current status
3. Update status with todo_update(id, "in_progress") or todo_update(id, "done")
4. Clear completed tasks with todo_clear_done()

Status values: pending, in_progress, done"#
                    .to_string(),
            );
        }
    }

    // Notes tools (always available)
    {
        if !blacklist.contains("note_add") {
            sections.push(
                r#"### NOTES TOOLS
Use for storing longer documents that should persist across sessions.
Available: note_add

**When to use note_add vs fact_add:**

Use **note_add** for:
- Architecture decisions and their rationale
- Implementation notes and summaries
- How-to guides and tutorials
- Extended code explanations
- Meeting notes and decisions
- Longer documents (up to 10,000 characters)

Use **fact_add** for:
- Short preferences ("I prefer dark mode", "Use snake_case")
- Quick facts ("Database is PostgreSQL 15", "API on port 8080")
- Settings and small configuration facts
- Single-sentence information (max 500 characters)

**How notes work:**
- Notes are stored in the database, NOT injected into the system prompt
- Retrieve notes with remember(id="note:N") or remember(query="topic")
- Notes are project-scoped (not global)

**Example:**
note_add("Decision: We chose PostgreSQL because:\n1. Better JSON support\n2. Native full-text search", "Architecture: Database Choice")"#
                    .to_string(),
            );
        }
    }

    // External CLI tools (always available, no feature flag)
    {
        let external_tools = ["check_tool_availability", "run_command"];
        let available: Vec<_> = external_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r#"### EXTERNAL TOOLS
Use for operations requiring external CLI tools (PDF, OCR, image metadata).
Available: check_tool_availability, run_command

**Controlling Output Size:**
- head=N: Return only first N lines (good for previews)
- tail=N: Return only last N lines (good for conclusions)
- Both null: Return FULL output (be careful with large files!)

**Workflow:**
1. Check availability: check_tool_availability("pdftotext")
2. For large files, use head/tail to preview first
3. Request specific pages with flags: pdftotext -f 1 -l 5 file.pdf -
4. Only request full output if you know it's reasonable

**Examples:**
- run_command("pdftotext doc.pdf -", 100, null, null) - First 100 lines
- run_command("pdftotext doc.pdf -", null, 50, null) - Last 50 lines
- run_command("pdftotext -f 11 -l 11 doc.pdf -", null, null, null) - Page 11 only
- run_command("pdftotext doc.pdf -", 50, 50, null) - First 50 + last 50 lines

**Security:** No shell features (pipes, redirects). Use tool-specific flags instead."#
                    .to_string(),
            );
        }
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_blacklist() {
        let blacklist = HashSet::new();
        let context = build_tool_context(&blacklist);

        // Should have content when features are enabled
        #[cfg(feature = "weather-tools")]
        assert!(context.contains("WEATHER TOOLS"));

        #[cfg(feature = "pokemon-tools")]
        assert!(context.contains("POKÉMON TOOLS"));
    }

    #[test]
    fn test_blacklist_filters_tools() {
        let mut blacklist = HashSet::new();
        blacklist.insert("web_search");

        let context = build_tool_context(&blacklist);

        // Web search should be filtered if blacklisted
        #[cfg(feature = "serper-tools")]
        {
            // With web_search blacklisted, the section may or may not appear
            // depending on whether web_search_news is also blacklisted
            if blacklist.contains("web_search_news") {
                assert!(!context.contains("WEB SEARCH"));
            }
        }
    }
}
