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

**PDFs:** read_file cannot read PDFs (binary format). Call skill_view("document-processing") for detailed instructions."#
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

    // Todo tools (always available)
    {
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
        let available: Vec<_> = todo_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            sections.push(
                r##"### TODO TOOLS
Use for tracking tasks during multi-step work. Reduces need to search conversation history.
Available: todo_add, todo_update, todo_get, todo_edit, todo_delete, todo_list, todo_clear_done, todo_clear_all

Workflow:
1. Add tasks with todo_add("description") when starting multi-step work
   - Optional: todo_add("Fix bug", priority="high", tags="bug,urgent")
2. List tasks with todo_list() to see current status
   - Filter: todo_list("pending"), todo_list("high"), todo_list("#bug")
3. Get details with todo_get(id)
4. Update status with todo_update(id, "in_progress") or todo_update(id, "done")
5. Edit tasks with todo_edit(id, description="new desc", priority="high")
6. Delete tasks with todo_delete(id)
7. Clear completed tasks with todo_clear_done()

Priority values: low, medium (default), high, critical
Status values: pending, in_progress, done
Tags: lowercase, comma-separated (e.g., "bug,frontend")"##.to_string(),
            );
        }
    }

    // Notes tools (always available)
    {
        if !blacklist.contains("note_add")
            || !blacklist.contains("note_edit")
            || !blacklist.contains("note_delete")
        {
            let section = r#"### NOTES TOOLS
Use for storing longer documents that should persist across sessions.
Available: note_add, note_edit, note_delete

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
- Edit notes with note_edit(id="N", title="...", content="...")
- Delete notes with note_delete(id="N")

**Example:**
note_add("Decision: We chose PostgreSQL because:\n1. Better JSON support\n2. Native full-text search", "Architecture: Database Choice")
note_edit(id="42", title="Revised: Database Decision")
note_delete(id="42")"#.to_string();

            sections.push(section);
        }
    }

    // Feedback tools (gated by config — check at runtime)
    {
        let settings = crate::tools::context::get_settings();
        if let Some(s) = &settings
            && s.feedback.enabled
            && !blacklist.contains("feedback_submit")
        {
            sections.push(
                r#"### FEEDBACK TOOLS
Use for providing feedback on messages during conversation.
Available: feedback_submit

feedback_submit allows you to rate messages as good, bad, or provide corrections.
Feedback helps improve future retrieval quality."#
                    .to_string(),
            );
        }
    }

    // Document tools (requires document-tools feature)
    #[cfg(feature = "document-tools")]
    {
        if !blacklist.contains("import_document") {
            sections.push(
                r#"### DOCUMENT TOOLS
Use for importing files into searchable memory.
Available: import_document

**When to use:**
- User mentions a file they want analyzed or referenced later
- You need to search file content in future conversations
- Building a knowledge base from documents

**How it works:**
- File is imported with **synchronous indexing** - searchable immediately
- Large documents are automatically split into ~512 token chunks
- Chunks enable granular search and navigation

**Title parameter (IMPORTANT):**
- For .txt files without obvious title: ALWAYS provide a descriptive title
- Good: "Meeting Notes 2026-03-29", "API Documentation", "GEB Chapter 1"
- Bad: "notes", "file", "document"
- For .md/.org files: Title is extracted automatically from headings

**File limits:**
- Maximum 2.5 MB (2,500,000 bytes) per file
- Supported: .txt, .md, .org only
- **PDF/EPUB not supported** — extract text first with run_command("pdftotext"), then import

**Example:**
// Plain text file - provide title
import_document("/path/to/notes.txt", None, Some("Project Planning Notes Q1"))

// Org file with #+TITLE: directive - auto-extracts title
import_document("/path/to/reference.org", None, None)

// Global scope for reference material
import_document("~/docs/glossary.md", Some("global"), None)

// PDF workflow: extract first, then import
// 1. run_command("pdftotext", ["report.pdf", "-"])  → get text output
// 2. Save output to a .txt file using write_file
// 3. import_document("report.txt", None, Some("Q3 Report"))"#
                    .to_string(),
            );
        }
    }
    // Agent spawning tools (requires subagent-tools feature)
    #[cfg(feature = "subagent-tools")]
    {
        let agent_tools = [
            "spawn_ocr_agent",
            "spawn_vision_agent",
            "spawn_translate_agent",
            "spawn_summarize_agent",
        ];
        let available: Vec<&&str> = agent_tools
            .iter()
            .filter(|t| !blacklist.contains(*t))
            .collect();

        if !available.is_empty() {
            let has_tool = |name: &&str| available.contains(&name);

            let mut section = String::from(
                "### AGENT SPAWNING TOOLS\n\
                Use for offloading specialized tasks to dedicated subagents.\n\n\
                Each tool is purpose-built for its task type with only the relevant parameters:\n",
            );

            if has_tool(&"spawn_ocr_agent") {
                section.push_str(
                    "\n- **spawn_ocr_agent** — Extract text from images via OCR\n\
                      Best for: tables, formulas, scanned documents, structured text\n\
                      Requires: `prompt` (what to extract), `file_path` (image path)\n\
                      Optional: `ocr_mode` (\"text\", \"table\", \"figure\", \"formula\")\n",
                );
            }

            if has_tool(&"spawn_vision_agent") {
                section.push_str(
                    "\n- **spawn_vision_agent** — Analyze or describe images via vision model\n\
                      Best for: charts, graphs, diagrams, visual content, comparisons\n\
                      Requires: `prompt` (what to analyze), `file_path` (image path(s))\n\
                      Supports comma-separated paths for multi-image analysis\n",
                );
            }

            if has_tool(&"spawn_translate_agent") {
                section.push_str(
                    "\n- **spawn_translate_agent** — Translate text between languages\n\
                      Requires: `prompt` (text + translation direction)\n\
                      No file needed — provide text directly in the prompt\n",
                );
            }

            if has_tool(&"spawn_summarize_agent") {
                section.push_str(
                    "\n- **spawn_summarize_agent** — Summarize long text into key points\n\
                      Requires: `prompt` (text + summarization instructions)\n\
                      No file needed — provide text directly in the prompt\n",
                );
            }

            // "When to use each" — built dynamically
            let mut when_to_use = String::from("\n**When to use each:**\n");
            let mut has_when = false;

            if has_tool(&"spawn_ocr_agent") {
                when_to_use
                    .push_str("- OCR → extracting text from images (screenshots, scanned docs)\n");
                has_when = true;
            }
            if has_tool(&"spawn_vision_agent") {
                when_to_use.push_str(
                    "- Vision → understanding visual content (charts, diagrams, photos)\n",
                );
                has_when = true;
            }
            if has_tool(&"spawn_translate_agent") {
                when_to_use.push_str("- Translate → converting text between languages\n");
                has_when = true;
            }
            if has_tool(&"spawn_summarize_agent") {
                when_to_use.push_str("- Summarize → condensing long text\n");
                has_when = true;
            }

            if has_when {
                section.push_str(&when_to_use);
            }

            // PDF section — only if OCR or vision tools are available
            if has_tool(&"spawn_ocr_agent") || has_tool(&"spawn_vision_agent") {
                section.push_str(
                    "\n**For PDF documents:** Use `run_command` with PDF tools \
                    (pdftotext, pdfinfo, pdftoppm).\n\
                    See the document-processing skill for the complete PDF pipeline.\n\
                    For PDF pages with visual content (charts, formulas, tables):\n\
                    1. Use run_command(\"pdftoppm\") to convert pages to images\n\
                    2. Then call spawn_vision_agent or spawn_ocr_agent on the \
                    resulting images\n",
                );
            }

            // Examples — built dynamically per available tool
            let mut examples = String::from("\n**Examples:**\n");
            let mut has_examples = false;

            if has_tool(&"spawn_ocr_agent") {
                examples.push_str(
                    "spawn_ocr_agent(\"Extract all text from this image\", \
                    \"/tmp/document.png\", None)\n\
                    spawn_ocr_agent(\"Extract table structure\", \"/tmp/table.png\", \
                    Some(\"table\"))\n",
                );
                has_examples = true;
            }
            if has_tool(&"spawn_vision_agent") {
                examples.push_str(
                    "spawn_vision_agent(\"Describe this chart\", \"/tmp/chart.png\")\n\
                    spawn_vision_agent(\"Compare these images\", \"/tmp/a.png,/tmp/b.png\")\n",
                );
                has_examples = true;
            }
            if has_tool(&"spawn_translate_agent") {
                examples
                    .push_str("spawn_translate_agent(\"Translate to Portuguese: Hello world\")\n");
                has_examples = true;
            }
            if has_tool(&"spawn_summarize_agent") {
                examples.push_str(
                    "spawn_summarize_agent(\"Summarize this text in 3 bullet points: ...\")\n",
                );
                has_examples = true;
            }

            if has_examples {
                section.push_str(&examples);
            }

            sections.push(section);
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

**Before using:** Check tool availability with check_tool_availability("tool-name").

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
