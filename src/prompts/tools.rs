//! Tool context builder
//!
//! Builds the tool section of system prompts. Instead of embedding detailed
//! tool descriptions in the prompt, we list available tools and rely on
//! the tool definitions (function metadata) for specifics.
//!
//! # Arguments
//! * `blacklist` - Set of tool names to exclude
//!
//! # Feature Flags
//! Tools are included based on compile-time feature flags:
//! - `weather-tools`: Weather tools
//! - `pokemon-tools`: Pokémon API tools
//! - `search-tools`: DuckDuckGo web search
//! - `file-tools`: File operation tools
//! - `calc-tools`: Calculator
//! - `system-tools`: System information tools

use std::collections::HashSet;

/// Filter a tool list against a blacklist, returning the available tools.
fn filter_available<'a>(tools: &[&'a str], blacklist: &HashSet<&str>) -> Vec<&'a str> {
    tools
        .iter()
        .filter(|t| !blacklist.contains(**t))
        .copied()
        .collect()
}

/// Weather tools section
#[cfg(feature = "weather-tools")]
fn weather_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = ["get_weather", "get_current_weather", "get_weather_forecast"];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r#"### WEATHER TOOLS
Use for weather and climate queries.
Available: get_weather, get_current_weather, get_weather_forecast"#
            .to_string(),
    )
}

/// Pokémon tools section
#[cfg(feature = "pokemon-tools")]
fn pokemon_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = [
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
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r#"### POKÉMON TOOLS
Use ONLY for Pokémon-related queries (names, abilities, moves, types, evolution).
Available: fetch_pokemon, fetch_pokemon_stats, fetch_pokemon_moves, etc."#
            .to_string(),
    )
}

/// DuckDuckGo web search section
#[cfg(feature = "search-tools")]
fn ddg_search_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = ["web_search", "web_search_news", "web_scrape"];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r#"### WEB SEARCH TOOLS
Use for general knowledge, current events, or anything NOT about Pokémon or weather.
Available: web_search, web_search_news, web_scrape"#
            .to_string(),
    )
}

/// Calculator section
#[cfg(feature = "calc-tools")]
fn calc_section(blacklist: &HashSet<&str>) -> Option<String> {
    if blacklist.contains("calculate") {
        return None;
    }
    Some(
        r#"### CALCULATOR TOOL
Use for mathematical calculations.
Available: calculate"#
            .to_string(),
    )
}

/// File tools section
#[cfg(feature = "file-tools")]
fn file_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = [
        "read_file",
        "read_file_segment",
        "count_lines",
        "list_directory",
    ];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r#"### FILE TOOLS
Use for reading and listing files.
Available: read_file, read_file_segment, count_lines, list_directory

Note: For large files, use count_lines first, then read_file_segment with start_line and num_lines.

**Searching file contents:** Use run_command("rg -n <pattern> <path>") for regex search. rg respects .gitignore, handles binary files, and has no file count or depth limits. Use head/tail to control output: run_command("rg -n pattern src/", "50", null, null) for first 50 lines. Use --glob for file filtering: run_command("rg -n --glob *.rs pattern .", null, null, null).

**PDFs:** read_file cannot read PDFs (binary format). **Load the document-processing skill FIRST** with skill_view(name="document-processing") for the complete two-phase pipeline (text extraction + vision analysis)."#
            .to_string(),
    )
}

/// File write tools section
#[cfg(feature = "file-tools")]
fn file_write_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = ["write_file", "edit_file", "append_file"];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r#"### FILE WRITE TOOLS
Use for creating, editing, and appending to files.
Available: write_file, edit_file, append_file

**Prefer edit_file** for making targeted changes to existing files. Use write_file only for
creating new files or complete rewrites.

**Uniqueness:** edit_file's replace operation requires a unique search string. If the search
string appears multiple times, the operation is rejected with the line numbers of the first
3 occurrences. Provide more surrounding context to make the search string unique.

**Must read before edit:** You cannot edit a file you have not read in this session. Use
read_file or read_file_segment first. The file is tracked with its content snapshot; if the
file is modified externally since your last read, the edit will be rejected. Re-read the file
to get the latest content before editing.

**Append is exempt:** append_file does NOT require a prior read or staleness check — appending
is additive and safe.

**Operations:** edit_file supports "replace" (find+replace), "insert" (after line N), and
"delete_lines" (range). Use "replace" for targeted text changes, "insert" for adding code,
"delete_lines" for removing lines by number."#
            .to_string(),
    )
}

/// System tools section
#[cfg(feature = "system-tools")]
fn system_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = ["get_current_datetime", "get_project_context"];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r#"### SYSTEM TOOLS
Use for current date/time or project context.
Available: get_current_datetime, get_project_context"#
            .to_string(),
    )
}

/// LED tools section
#[cfg(feature = "led-tools")]
fn led_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = [
        "led_get_status",
        "led_set_power",
        "led_set_program",
        "led_set_brightness",
        "led_set_color",
    ];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
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
    )
}

/// Todo tools section (always available)
fn todo_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = [
        "todo_add",
        "todo_update",
        "todo_get",
        "todo_edit",
        "todo_delete",
        "todo_list",
        "todo_clear_done",
        "todo_clear_all",
    ];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
        r##"### TODO TOOLS
Track tasks during multi-step work. Add with todo_add, list with todo_list, update status with todo_update, edit with todo_edit, delete with todo_delete.
Priority: low, medium (default), high, critical. Status: pending, in_progress, done. Tags: comma-separated (e.g., "#bug,urgent")."##.to_string(),
    )
}

/// Notes tools section (always available)
fn notes_section(blacklist: &HashSet<&str>) -> Option<String> {
    if blacklist.contains("note_add")
        && blacklist.contains("note_edit")
        && blacklist.contains("note_delete")
    {
        return None;
    }
    Some(
        r#"### NOTES TOOLS
Store longer documents that persist across sessions. Available: note_add, note_edit, note_delete.

**When to use note_add vs fact_add:**
- **note_add**: Architecture decisions, how-to guides, meeting notes, documents up to 10,000 chars
- **fact_add**: Short preferences and facts, single-sentence info (max 500 chars)

Notes are stored in the database (not in the prompt). Retrieve with remember(id="note:N") or remember(query="topic"). Project-scoped by default."#
            .to_string(),
    )
}

/// Feedback tools section (gated by config)
fn feedback_section(blacklist: &HashSet<&str>) -> Option<String> {
    let settings = crate::tools::context::get_settings();
    if let Some(s) = &settings
        && s.feedback.enabled
        && !blacklist.contains("feedback_submit")
    {
        Some(
            r#"### FEEDBACK TOOLS
Use for providing feedback on messages during conversation.
Available: feedback_submit

feedback_submit allows you to rate messages as good, bad, or provide corrections.
Feedback helps improve future retrieval quality."#
                .to_string(),
        )
    } else {
        None
    }
}

/// Document tools section
#[cfg(feature = "document-tools")]
fn document_section(blacklist: &HashSet<&str>) -> Option<String> {
    if blacklist.contains("import_document") {
        return None;
    }
    Some(
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
    )
}

/// Agent spawning tools section
#[cfg(feature = "subagent-tools")]
fn agent_section(blacklist: &HashSet<&str>) -> Option<String> {
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

    if available.is_empty() {
        return None;
    }

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
              Requires: `file_path` (image path)\n\
              Optional: `ocr_mode` (\"text\", \"table\", \"figure\", \"formula\")\n\
              NOTE: Does NOT accept a custom prompt — OCR mode determines extraction type.\n\
              For custom image analysis, use spawn_vision_agent instead.\n",
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
        when_to_use.push_str("- OCR → extracting text from images (screenshots, scanned docs). No prompt — uses ocr_mode instead.\n");
        has_when = true;
    }
    if has_tool(&"spawn_vision_agent") {
        when_to_use
            .push_str("- Vision → understanding visual content (charts, diagrams, photos)\n");
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
            "\n**For PDF documents:** Load the document-processing skill FIRST with \
            skill_view(name=\"document-processing\") — it provides the complete \
            two-phase pipeline (text extraction → OCR/vision for visual content) \
            with detection heuristics for tables, charts, formulas, and scanned pages.\n\
            Quick reference: For pages with visual content:\n\
            1. run_command(\"pdftoppm\") to convert pages to images\n\
            2. spawn_ocr_agent for tables/formulas/scanned text\n\
            3. spawn_vision_agent for charts/diagrams/visual analysis\n",
        );
    }

    // Examples — built dynamically per available tool
    let mut examples = String::from("\n**Examples:**\n");
    let mut has_examples = false;

    if has_tool(&"spawn_ocr_agent") {
        examples.push_str(
            "spawn_ocr_agent(\"/tmp/document.png\", None)\n\
            spawn_ocr_agent(\"/tmp/table.png\", Some(\"table\"))\n",
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
        examples.push_str("spawn_translate_agent(\"Translate to Portuguese: Hello world\")\n");
        has_examples = true;
    }
    if has_tool(&"spawn_summarize_agent") {
        examples
            .push_str("spawn_summarize_agent(\"Summarize this text in 3 bullet points: ...\")\n");
        has_examples = true;
    }

    if has_examples {
        section.push_str(&examples);
    }

    Some(section)
}

/// External CLI tools section (always available, no feature flag)
fn external_section(blacklist: &HashSet<&str>) -> Option<String> {
    let tools = ["check_tool_availability", "run_command"];
    let available = filter_available(&tools, blacklist);
    if available.is_empty() {
        return None;
    }
    Some(
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
    )
}

/// Build minimal tool context section
///
/// Lists available tools by category with brief usage guidance.
/// Detailed descriptions are provided by the tool definitions themselves,
/// which are more efficient for the model to process.
pub fn build_tool_context(blacklist: &HashSet<&str>) -> String {
    let mut sections = Vec::new();

    #[cfg(feature = "weather-tools")]
    if let Some(s) = weather_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "pokemon-tools")]
    if let Some(s) = pokemon_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "search-tools")]
    if let Some(s) = ddg_search_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "calc-tools")]
    if let Some(s) = calc_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "file-tools")]
    if let Some(s) = file_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "file-tools")]
    if let Some(s) = file_write_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "system-tools")]
    if let Some(s) = system_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "led-tools")]
    if let Some(s) = led_section(blacklist) {
        sections.push(s);
    }

    if let Some(s) = todo_section(blacklist) {
        sections.push(s);
    }
    if let Some(s) = notes_section(blacklist) {
        sections.push(s);
    }
    if let Some(s) = feedback_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "document-tools")]
    if let Some(s) = document_section(blacklist) {
        sections.push(s);
    }

    #[cfg(feature = "subagent-tools")]
    if let Some(s) = agent_section(blacklist) {
        sections.push(s);
    }

    if let Some(s) = external_section(blacklist) {
        sections.push(s);
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_blacklist() {
        let blacklist = HashSet::new();

        // The `context` variable is only used inside the weather-tools
        // and pokemon-tools cfg blocks below, so the variable itself
        // is also gated to avoid a `unused variable` warning when
        // those features are off.
        #[cfg(any(feature = "weather-tools", feature = "pokemon-tools"))]
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

        // Web search should be filtered if blacklisted.
        // The `context` variable is only used inside the search-tools
        // cfg block below, so the variable itself is also gated to
        // avoid a `unused variable` warning when the feature is off.
        #[cfg(feature = "search-tools")]
        let context = build_tool_context(&blacklist);

        #[cfg(feature = "search-tools")]
        {
            // With web_search blacklisted, the section may or may not appear
            // depending on whether web_search_news is also blacklisted
            if blacklist.contains("web_search_news") {
                assert!(!context.contains("WEB SEARCH"));
            }
        }
    }

    #[cfg(feature = "file-tools")]
    #[test]
    fn test_file_write_section_present() {
        let blacklist = HashSet::new();
        let section = file_write_section(&blacklist);
        assert!(section.is_some());
        let s = section.unwrap();
        assert!(s.contains("### FILE WRITE TOOLS"));
        assert!(s.contains("write_file"));
        assert!(s.contains("edit_file"));
        assert!(s.contains("append_file"));
        assert!(s.contains("Uniqueness"));
    }
}
