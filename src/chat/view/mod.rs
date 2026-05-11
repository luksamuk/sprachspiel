//! View abstraction layer for chat REPL
//!
//! This module provides the `ChatView` trait for abstracting output rendering,
//! enabling future migration from terminal output to alternative rendering (e.g., TUI).
//!
//! # Architecture
//!
//! ```text
//! repl.rs (coordinator)
//!     ↓ uses
//! ChatView (trait)
//!     ↓ implemented by
//! TerminalView (current) ─── TuiView (future)
//! ```
//!
//! NOTE: The `ChatView` trait is intentionally kept for future TUI implementation.
//! See AGENTS.md "TUI Preparation Code Policy" for details.

#![expect(dead_code)]

/// Number of lines in the status bar (divisor, content, divisor)
/// Used by ANSI clear codes in repl.rs to remove status bar before user input
pub const STATUS_BAR_LINES: usize = 3;

// ANSI color codes for banner styling
pub mod colors {
    pub const CYAN: &str = "\x1B[36m";
    pub const YELLOW: &str = "\x1B[33m";
    pub const BOLD: &str = "\x1B[1m";
    pub const DIM: &str = "\x1B[2m";
    pub const RESET: &str = "\x1B[0m";
    pub const BOLD_CYAN: &str = "\x1B[1;36m";
    pub const BOLD_YELLOW: &str = "\x1B[1;33m";
    pub const GREEN: &str = "\x1B[32m";
    pub const RED: &str = "\x1B[31m";
}

/// ASCII art logo using toilet "future" font (pre-rendered)
/// SPRACH in gold (#220), SPIEL in cyan (#45)
const BANNER_LOGO: &str = "\
\x1B[38;5;220m┏━┓┏━┓┏━┓┏━┓┏━╸╻ ╻\x1B[0m\x1B[38;5;45m┏━┓┏━┓╻┏━╸╻  \x1B[0m\n\
\x1B[38;5;220m┗━┓┣━┛┣┳┛┣━┫┃  ┣━┫\x1B[0m\x1B[38;5;45m┗━┓┣━┛┃┣╸ ┃  \x1B[0m\n\
\x1B[38;5;136m┗━┛╹  ╹┗╸╹ ╹┗━╸╹ ╹\x1B[0m\x1B[0;36m┗━┛╹  ╹┗━╸┗━╸\x1B[0m";

/// Extended mind braille art - generated from extended-mind.png via braille_art.py
/// Connected mind representing tools/memory/Zettelkasten
/// Generated: python3 braille_art.py extended-mind.png -w 39 --color
const EXTENDED_MIND_ART: [&str; 14] = [
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;245;213;122m⢀\x1B[38;2;237;216;142m⣤\x1B[38;2;255;248;123m⡀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;244;232;147m⢀\x1B[38;2;228;195;100m⣾\x1B[38;2;252;249;180m⣿\x1B[38;2;235;210;90m⡿⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;182;134;51m⢀\x1B[38;2;248;235;159m⣠\x1B[38;2;217;191;113m⠾\x1B[38;2;238;219;134m⠋⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀\x1B[38;2;212;169;24m⢠\x1B[38;2;250;243;171m⣶\x1B[38;2;246;237;157m⣶\x1B[38;2;188;136;4m⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;220;195;120m⣠\x1B[38;2;237;216;137m⡴\x1B[38;2;226;204;128m⠛\x1B[38;2;215;178;83m⠁⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀\x1B[38;2;213;169;14m⠈\x1B[38;2;244;234;119m⠻\x1B[38;2;238;220;99m⠟\x1B[38;2;225;187;85m⠓\x1B[38;2;244;231;160m⠦\x1B[38;2;228;205;133m⣤\x1B[38;2;218;189;110m⣀⠀⠀⠀⠀⠀\x1B[38;2;211;250;253m⢀\x1B[38;2;195;228;230m⣤\x1B[38;2;209;234;236m⣶\x1B[38;2;184;224;226m⢟\x1B[38;2;178;222;225m⣛\x1B[38;2;173;219;223m⣽\x1B[38;2;169;221;225m⣿\x1B[38;2;219;243;245m⠶\x1B[38;2;173;215;218m⣖\x1B[38;2;131;202;207m⣦⠀\x1B[38;2;217;198;105m⠰\x1B[38;2;232;212;131m⠞\x1B[38;2;255;249;173m⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;240;226;148m⠉\x1B[38;2;230;209;133m⠛\x1B[38;2;229;208;128m⠶\x1B[38;2;209;187;99m⣤⠀\x1B[38;2;194;231;234m⣼\x1B[38;2;205;233;235m⣿\x1B[38;2;198;233;235m⢫\x1B[38;2;218;238;240m⣶\x1B[38;2;193;228;231m⣿\x1B[38;2;220;240;241m⡿\x1B[38;2;186;223;227m⣛\x1B[38;2;191;223;226m⣯\x1B[38;2;163;209;213m⣸\x1B[38;2;233;246;248m⣿\x1B[38;2;204;232;234m⡿\x1B[38;2;191;229;233m⣿\x1B[38;2;149;204;208m⣷\x1B[38;2;123;203;210m⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;141;204;207m⢠\x1B[38;2;184;229;232m⣾\x1B[38;2;222;242;243m⣿\x1B[38;2;174;215;218m⢟\x1B[38;2;184;222;225m⣻\x1B[38;2;240;250;251m⣿\x1B[38;2;189;225;229m⣿\x1B[38;2;199;228;231m⣿\x1B[38;2;230;244;245m⡿\x1B[38;2;189;227;230m⣿\x1B[38;2;249;253;253m⣿\x1B[38;2;216;240;243m⡿\x1B[38;2;214;234;236m⣿\x1B[38;2;181;221;225m⡽\x1B[38;2;248;252;253m⠿\x1B[38;2;161;213;219m⢿\x1B[38;2;196;227;229m⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;217;249;250m⢸\x1B[38;2;159;211;215m⣿\x1B[38;2;183;222;225m⡟\x1B[38;2;188;224;227m⢿\x1B[38;2;224;242;243m⣿\x1B[38;2;205;231;234m⡿\x1B[38;2;183;222;225m⢇\x1B[38;2;240;251;251m⣴\x1B[38;2;242;250;251m⣶\x1B[38;2;193;226;230m⣿\x1B[38;2;246;252;252m⠿\x1B[38;2;166;212;215m⢏\x1B[38;2;209;232;233m⣭\x1B[38;2;186;223;227m⣵\x1B[38;2;231;245;246m⣿\x1B[38;2;209;236;238m⣿\x1B[38;2;168;220;225m⣿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;94;168;175m⠈\x1B[38;2;227;248;249m⠻\x1B[38;2;180;218;220m⣿\x1B[38;2;174;220;224m⢺\x1B[38;2;243;251;251m⣿\x1B[38;2;229;244;246m⣿\x1B[38;2;219;239;241m⣿\x1B[38;2;201;229;232m⣿\x1B[38;2;214;235;236m⠿\x1B[38;2;186;223;226m⣵\x1B[38;2;228;243;244m⣿\x1B[38;2;216;236;238m⣿\x1B[38;2;190;227;230m⢟\x1B[38;2;197;226;229m⣟\x1B[38;2;199;231;233m⣫\x1B[38;2;226;244;245m⣿\x1B[38;2;183;225;228m⠏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;230;203;118m⢀\x1B[38;2;226;206;125m⣤\x1B[38;2;216;191;93m⠆⠀⠀\x1B[38;2;197;233;236m⠙\x1B[38;2;227;249;251m⠛\x1B[38;2;215;242;244m⠛\x1B[38;2;237;255;255m⠁\x1B[38;2;185;223;226m⣿\x1B[38;2;236;248;249m⣿\x1B[38;2;195;229;232m⣿\x1B[38;2;204;230;232m⣿\x1B[38;2;218;240;241m⠿\x1B[38;2;219;238;239m⣛\x1B[38;2;218;239;241m⣫\x1B[38;2;158;209;212m⡵\x1B[38;2;220;191;100m⢀\x1B[38;2;184;147;73m⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;231;205;128m⢀\x1B[38;2;228;208;132m⣤\x1B[38;2;230;209;135m⠞\x1B[38;2;242;226;144m⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;195;240;242m⠉\x1B[38;2;213;249;251m⠉\x1B[38;2;127;200;205m⠁\x1B[38;2;122;228;234m⢎\x1B[38;2;208;237;239m⠛\x1B[38;2;224;245;246m⠋\x1B[38;2;80;166;173m⠁\x1B[38;2;193;155;73m⠈\x1B[38;2;244;229;158m⠙\x1B[38;2;227;203;128m⠶\x1B[38;2;229;208;133m⣤\x1B[38;2;249;232;156m⡀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀\x1B[38;2;237;214;133m⢀\x1B[38;2;228;208;132m⣤\x1B[38;2;232;212;134m⠞\x1B[38;2;241;226;144m⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;222;195;118m⠙\x1B[38;2;235;214;135m⠳\x1B[38;2;235;213;134m⢦\x1B[38;2;244;231;146m⣀\x1B[38;2;193;128;26m⢀\x1B[38;2;255;245;140m⣀\x1B[38;2;242;204;37m⡀⠀\x1B[0m",
    "⠀\x1B[38;2;232;209;100m⣾\x1B[38;2;252;251;199m⣿\x1B[38;2;228;195;87m⣷\x1B[38;2;235;216;127m⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;248;233;130m⠈\x1B[38;2;240;211;129m⢻\x1B[38;2;252;252;200m⣿\x1B[38;2;244;227;100m⡿⠀\x1B[0m",
    "⠀\x1B[38;2;255;246;131m⠈\x1B[38;2;237;216;107m⠛\x1B[38;2;250;223;60m⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;232;195;38m⠉\x1B[38;2;193;136;0m⠁⠀\x1B[0m",
];

// TUI Migration
// When implementing ratatui.rs TUI:
// - Add methods to trait for new rendering needs
// - Implement `TuiView` struct in `src/chat/view/tui.rs`
// - Update `repl.rs` to use the new implementation
// IMPORTANT: Review and remove any dead code after TUI is implemented.

mod terminal;

pub use terminal::TerminalView;

// Re-export TokenMetrics from core for consumers of this module
pub use crate::chat::core::TokenMetrics;

/// Abstraction for output rendering in the chat REPL
///
/// This trait enables the REPL to work with different output backends:
/// - `TerminalView`: Current implementation using println!/eprintln!
/// - `TuiView`: Future implementation for ratatui.rs TUI
///
/// **Note:** This trait is part of the TUI migration architecture (see AGENTS.md).
/// It provides the abstraction layer for switching between terminal output and
/// future TUI rendering. Currently used via TerminalView in repl.rs.
///
/// # Example
///
/// ```ignore
/// use chat::view::ChatView;
///
/// let mut view = TerminalView::new();
/// view.show_welcome(&session, &model_config, &capabilities);
/// view.show_assistant_response(&content, thinking, &metrics);
/// view.show_error("Something went wrong");
/// ```
pub trait ChatView {
    /// Display a system message (info, status, welcome)
    ///
    /// Used for:
    /// - Welcome banner on startup
    /// - Status messages (model switched, tools toggled)
    /// - Command results (compact complete, etc.)
    fn show_system(&mut self, message: &str);

    /// Display an error message
    ///
    /// Errors are typically shown in red/bold to catch user attention.
    fn show_error(&mut self, error: &str);

    /// Display an assistant response with optional thinking content
    ///
    /// For models with thinking support (e.g., DeepSeek R1), the thinking
    /// content is displayed separately (typically dimmed/italic) before
    /// the main response.
    ///
    /// # Arguments
    ///
    /// * `content` - The main response content (after thinking tags removed)
    /// * `thinking` - Optional thinking content to display first
    fn show_assistant_response(&mut self, content: &str, thinking: Option<&str>);

    /// Display token usage metrics
    ///
    /// Shows prompt tokens, response tokens, and total after a response.
    fn show_token_metrics(&mut self, metrics: &TokenMetrics);

    /// Display a context warning
    ///
    /// Used when context window is getting full (72%+ or 80%+ thresholds).
    /// Should be visually distinct (yellow/warning color).
    fn show_context_warning(&mut self, percent: u8, message: &str);

    /// Display compact progress indicator
    ///
    /// Shows when auto-compaction is in progress.
    fn show_compact_progress(&mut self, message: &str);

    /// Display a compact complete message
    ///
    /// Shows after compaction finishes, with count of messages compacted.
    fn show_compact_complete(
        &mut self,
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    );

    /// Display a command output result
    ///
    /// Dispatches rendering based on the `CommandOutput` variant.
    /// Each variant is rendered with appropriate styling:
    /// - `Info` → dim/cyan system message
    /// - `Success` → green with ✓ icon
    /// - `Warning` → yellow with ⚠ icon
    /// - `Error` → red with ✗ icon
    /// - `Progress` → yellow with ⏳ icon
    /// - Structured variants → formatted displays
    ///
    /// This method is the primary entry point for command output rendering,
    /// enabling clean decoupling of command logic from presentation.
    fn show_command_output(&mut self, output: &crate::chat::CommandOutput);

    /// Display multiple command outputs in sequence
    ///
    /// Convenience method that calls `show_command_output` for each item.
    /// Commands return `Vec<CommandOutput>` to support multi-part results
    /// (e.g., a warning followed by a success message).
    fn show_command_outputs(&mut self, outputs: &[crate::chat::CommandOutput]) {
        for output in outputs {
            self.show_command_output(output);
        }
    }
}

/// Welcome information for display
///
/// Contains all the data needed to render a welcome banner.
/// This is deliberately a struct to allow different rendering strategies
/// (ASCII box for terminal, widgets for TUI).
pub struct WelcomeInfo {
    pub model_id: String,
    pub tools_enabled: bool,
    pub think_enabled: bool,
    pub vision_enabled: bool,
    pub sandbox_status: String,
    pub project: String,
    pub session_name: String,
    pub is_anonymous: bool,
    pub version: String,
    pub server_url: String,
    pub fact_count: i64,
    pub note_count: i64,
    pub doc_count: i64,
    pub skill_count: usize,
}

impl WelcomeInfo {
    /// Format the welcome banner with ASCII art and session info
    pub fn to_boxed_string(&self) -> String {
        let mut output = String::new();
        output.push('\n');

        let session_lines = self.format_session_lines();

        output.push_str(BANNER_LOGO);
        output.push('\n');
        output.push_str(&format!("{}{}\n", colors::DIM, "─".repeat(80)));
        output.push('\n');

        let art_visual_widths: Vec<usize> = EXTENDED_MIND_ART
            .iter()
            .map(|line| strip_ansi_width(line))
            .collect();

        let max_art_width = art_visual_widths.iter().max().copied().unwrap_or(0);

        for (i, art_line) in EXTENDED_MIND_ART.iter().enumerate() {
            let visual_width = art_visual_widths[i];
            let padding = max_art_width.saturating_sub(visual_width);

            if i < session_lines.len() {
                output.push_str(&format!(
                    "{}{}{}{}\n",
                    art_line,
                    " ".repeat(padding),
                    session_lines[i],
                    colors::RESET
                ));
            } else {
                output.push_str(&format!("{}\n", art_line));
            }
        }

        output.push('\n');
        output.push_str(&format!("{}{}\n", colors::DIM, "─".repeat(80)));

        output
    }

    /// Format only the help line (to be printed after all startup messages)
    pub fn help_line() -> String {
        let help_msg = "Type /help for commands, /quit to exit";
        let padding = (80_usize.saturating_sub(help_msg.len())) / 2;
        format!(
            "\n{}{}{}{}\n",
            " ".repeat(padding),
            colors::DIM,
            help_msg,
            colors::RESET
        )
    }

    /// Format session info lines for right-side display
    ///
    /// Order: most useful/frequently consulted first, static metadata last:
    /// Model → Server → Tools → Think → Vision → Sandbox → Project → Session → Version
    /// Then conditional: Facts, Notes, Docs, Skills
    fn format_session_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        let bc = colors::BOLD_CYAN;
        let d = colors::DIM;
        let r = colors::RESET;

        // Core identity
        lines.push(format!(
            "{}Model:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.model_id, 30),
            r
        ));

        lines.push(format!(
            "{}Server:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.server_url, 30),
            r
        ));

        // Capabilities
        let tools_status = if self.tools_enabled {
            "enabled"
        } else {
            "disabled"
        };
        lines.push(format!("{}Tools:{} {}{}{}", bc, r, d, tools_status, r));

        if self.think_enabled {
            lines.push(format!("{}Think:{} {}enabled{}", bc, r, d, r));
        }

        if self.vision_enabled {
            lines.push(format!("{}Vision:{} {}enabled{}", bc, r, d, r));
        }

        // Security
        lines.push(format!(
            "{}Sandbox:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.sandbox_status, 28),
            r
        ));

        // Context
        lines.push(format!(
            "{}Project:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.project, 30),
            r
        ));

        let session_display = if self.is_anonymous {
            "anonymous".to_string()
        } else {
            self.session_name.clone()
        };
        lines.push(format!(
            "{}Session:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&session_display, 30),
            r
        ));

        // Static metadata
        lines.push(format!("{}Version:{} {}{}{}", bc, r, d, self.version, r));

        // Content counts (only shown when > 0)
        if self.fact_count > 0 {
            lines.push(format!("{}Facts:{} {}{}{}", bc, r, d, self.fact_count, r));
        }

        if self.note_count > 0 {
            lines.push(format!("{}Notes:{} {}{}{}", bc, r, d, self.note_count, r));
        }

        if self.doc_count > 0 {
            lines.push(format!("{}Docs:{} {}{}{}", bc, r, d, self.doc_count, r));
        }

        // Skills (only when tools enabled and skills available)
        if self.tools_enabled && self.skill_count > 0 {
            lines.push(format!("{}Skills:{} {}{}{}", bc, r, d, self.skill_count, r));
        }

        lines
    }
}

/// Status bar information for display above prompt
///
/// Contains context usage, model name, and toggle indicators.
/// Rendered as a fixed-width (80 columns) bar above the prompt.
pub struct StatusBarInfo {
    pub model_name: String,
    pub used_tokens: usize,
    pub max_tokens: usize,
    pub percent: u8,
    pub think_enabled: bool,
    pub tools_enabled: bool,
}

impl StatusBarInfo {
    /// Format the status bar as a 3-line string (divisor, content, divisor)
    ///
    /// Format:
    /// ```text
    /// ────────────────────────────────────────────────────────────────────────────────
    ///  glm-5:cloud │ 47.2K/128K │ ████████░░░░ 37% │ 🧠🔧
    /// ────────────────────────────────────────────────────────────────────────────────
    /// ```
    ///
    /// Progress bar colors:
    /// - Green: < 50%
    /// - Yellow: 50-75%
    /// - Red: > 75%
    pub fn format_status_bar(&self) -> String {
        let mut output = String::new();

        let separator = "─".repeat(80);
        output.push_str(&format!("{}{}{}\n", colors::DIM, separator, colors::RESET));

        let content = self.format_content_line();
        output.push_str(&content);
        output.push('\n');

        output.push_str(&format!("{}{}{}\n", colors::DIM, separator, colors::RESET));

        output
    }

    fn format_content_line(&self) -> String {
        // Build content without ANSI colors first
        let mut content = String::new();

        // Model name (truncated to 20 chars)
        content.push_str(&truncate_str(&self.model_name, 20));

        // First separator
        content.push_str(" │ ");

        // Context usage: tokens
        let used_str = format_tokens(self.used_tokens);
        let max_str = format_tokens(self.max_tokens);
        content.push_str(&format!("{}/{}", used_str, max_str));

        // Second separator
        content.push_str(" │ ");

        // Progress bar with percentage (with colors)
        let bar_str = self.format_progress_bar();
        content.push_str(&bar_str);

        // Third separator
        content.push_str(" │ ");

        // Indicators (think/tools)
        if self.think_enabled {
            content.push('🧠');
        }
        if self.tools_enabled {
            content.push('🔧');
        }

        // Calculate visual width (excluding ANSI codes)
        let visual_width = strip_ansi_width(&content);

        // Truncate if too long, or add padding if too short
        // Use 77 to account for potential unicode width issues
        if visual_width > 77 {
            truncate_visual(&content, 77)
        } else {
            // Add padding to reach 77 columns
            let padding = 77 - visual_width;
            content.push_str(&" ".repeat(padding));
            content
        }
    }

    /// Format progress bar with percentage
    ///
    /// Example: `████████░░░░ 37%`
    fn format_progress_bar(&self) -> String {
        let bar_width = 12;
        let filled = ((self.percent as usize) * bar_width) / 100;
        let empty = bar_width.saturating_sub(filled);

        let bar_color = get_bar_color(self.percent);

        let mut bar = String::new();
        bar.push_str(bar_color);
        bar.push_str(&"█".repeat(filled));
        bar.push_str(&"░".repeat(empty));
        bar.push_str(colors::RESET);

        format!("{} {}%{}", bar, self.percent, colors::RESET)
    }

    /// Calculate visual width stripping ANSI codes
    fn calculate_visual_width(&self, s: &str) -> usize {
        strip_ansi_width(s)
    }
}

/// A single message in the recent context display
pub struct RecentMessage {
    /// Role label (e.g., "👤 User", "🤖 Assistant")
    pub role_label: String,
    /// Truncated message content
    pub content: String,
}

/// Recent context information for session resume display
///
/// Contains the last few exchanges (user+assistant pairs) from
/// the resumed session, shown after the welcome banner.
pub struct RecentContextInfo {
    /// Number of total messages in the session
    pub total_messages: usize,
    /// Recent exchanges in chronological order (oldest first)
    pub exchanges: Vec<(RecentMessage, Option<RecentMessage>)>,
}

impl RecentContextInfo {
    /// Format the recent context as a dimmed block below the banner.
    ///
    /// Shows the last few exchanges with role labels and truncated content.
    /// Returns an empty string if there are no exchanges to display.
    pub fn format_context_summary(&self) -> String {
        if self.exchanges.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        output.push_str(&format!(
            "{}{}Recent context ({} messages):{}\n",
            colors::DIM,
            colors::BOLD,
            self.total_messages,
            colors::RESET
        ));

        for (user_msg, assistant_msg) in &self.exchanges {
            // User message line
            output.push_str(&format!(
                "  {}{}{}:{} {}\n",
                colors::BOLD_CYAN,
                user_msg.role_label,
                colors::RESET,
                colors::DIM,
                user_msg.content
            ));

            // Assistant message line (if present)
            if let Some(asst) = assistant_msg {
                output.push_str(&format!(
                    "  {}{}{}:{} {}\n",
                    colors::BOLD_YELLOW,
                    asst.role_label,
                    colors::RESET,
                    colors::DIM,
                    asst.content
                ));
            }
        }

        output.push_str(colors::RESET);
        output
    }
}

/// Format tokens as human-readable string
///
/// Examples:
/// - 500 -> "500"
/// - 1500 -> "1.5K"
/// - 1500000 -> "1.5M"
fn format_tokens(tokens: usize) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Get progress bar color based on percentage
///
/// - Green: < 50%
/// - Yellow: 50-75%
/// - Red: > 75%
fn get_bar_color(percent: u8) -> &'static str {
    if percent < 50 {
        colors::GREEN
    } else if percent < 75 {
        colors::YELLOW
    } else {
        colors::RED
    }
}

/// Maximum length for each message line in the recent context display
pub const MAX_CONTEXT_LINE_LENGTH: usize = 80;

/// Truncate a string to a maximum length, adding ellipsis if truncated
pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Truncate a string to a maximum visual width (Unicode-aware, ANSI-aware)
/// Each Unicode character counts as 1 visual column, even multi-byte chars.
/// ANSI escape codes are preserved but not counted.
pub(crate) fn truncate_visual(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut width = 0;
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() && width <= max_width {
        if chars[i] == '\x1B' {
            // Include entire ANSI escape sequence
            result.push(chars[i]);
            i += 1;
            if i < chars.len() && chars[i] == '[' {
                result.push(chars[i]);
                i += 1;
                while i < chars.len() {
                    result.push(chars[i]);
                    i += 1;
                    if chars[i - 1].is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if chars[i] == '\n' {
            // Skip newlines
            i += 1;
        } else {
            // Count as 1 visual column
            if width < max_width {
                result.push(chars[i]);
            }
            width += 1;
            i += 1;
        }
    }

    result
}

/// Calculate visual width of a string, stripping ANSI escape codes
pub(crate) fn strip_ansi_width(s: &str) -> usize {
    let mut width = 0;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\x1B' {
            // Skip ANSI escape sequence
            i += 1;
            if i < chars.len() && chars[i] == '[' {
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    i += 1;
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if chars[i] == '\n' {
            i += 1;
        } else {
            width += 1;
            i += 1;
        }
    }

    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_info_formatting() {
        let info = WelcomeInfo {
            model_id: "qwen3.5:4b".to_string(),
            tools_enabled: true,
            think_enabled: true,
            vision_enabled: false,
            sandbox_status: "enabled (landlock)".to_string(),
            project: "my-project".to_string(),
            session_name: "default".to_string(),
            is_anonymous: false,
            version: "0.40.0".to_string(),

            server_url: "localhost:11434".to_string(),
            fact_count: 3,
            note_count: 2,
            doc_count: 1,
            skill_count: 4,
        };

        let output = info.to_boxed_string();
        assert!(output.contains("qwen3.5:4b"));
        assert!(output.contains("Project:"));
        assert!(output.contains("Session:"));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a very long string", 10), "this is...");
    }

    #[test]
    fn test_truncate_str_unicode() {
        // Regression test: byte-based slicing panicked on multibyte chars
        // Portuguese text with ç, ã, é — exact crash scenario from bug report
        let pt = "A conexão com Wittgenstein é perfeita";
        let result = truncate_str(pt, 20);
        assert!(result.ends_with("..."));
        assert!(!result.is_empty());

        // CJK characters (3 bytes each) - with max_len=7, we get 4 chars + ...
        // 7 - 3 = 4, so we should get 4 characters before ...
        let cjk = "你好世界Hello世界";
        let result = truncate_str(cjk, 7); // 7 chars = 4 Chinese + 3 ASCII, then ...
        assert!(result.ends_with("..."));
        assert!(result.starts_with("你好"));

        // String shorter than max_len should not be modified
        let short = "café";
        assert_eq!(truncate_str(short, 10), "café");

        // Boundary case: exactly max_len chars
        let exact = "abcdefghijklmno"; // 15 chars
        assert_eq!(truncate_str(exact, 15), "abcdefghijklmno");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(47200), "47.2K");
        assert_eq!(format_tokens(128000), "128.0K");
        assert_eq!(format_tokens(1500000), "1.5M");
    }

    #[test]
    fn test_get_bar_color() {
        assert_eq!(get_bar_color(0), colors::GREEN);
        assert_eq!(get_bar_color(49), colors::GREEN);
        assert_eq!(get_bar_color(50), colors::YELLOW);
        assert_eq!(get_bar_color(74), colors::YELLOW);
        assert_eq!(get_bar_color(75), colors::RED);
        assert_eq!(get_bar_color(100), colors::RED);
    }

    #[test]
    fn test_status_bar_info_formatting() {
        let info = StatusBarInfo {
            model_name: "glm-5:cloud".to_string(),
            used_tokens: 47200,
            max_tokens: 128000,
            percent: 37,
            think_enabled: true,
            tools_enabled: true,
        };

        let output = info.format_status_bar();
        assert!(output.contains("glm-5:cloud"));
        assert!(output.contains("47.2K"));
        assert!(output.contains("128.0K"));
        assert!(output.contains("37%"));
        assert!(output.contains("🧠"));
        assert!(output.contains("🔧"));
        assert!(output.contains(&"─".repeat(80)));
    }

    #[test]
    fn test_status_bar_info_no_indicators() {
        let info = StatusBarInfo {
            model_name: "llama3.1".to_string(),
            used_tokens: 500,
            max_tokens: 4096,
            percent: 12,
            think_enabled: false,
            tools_enabled: false,
        };

        let output = info.format_status_bar();
        assert!(!output.contains("🧠"));
        assert!(!output.contains("🔧"));
    }

    #[test]
    fn test_recent_context_info_empty() {
        let info = RecentContextInfo {
            total_messages: 0,
            exchanges: vec![],
        };
        assert!(info.format_context_summary().is_empty());
    }

    #[test]
    fn test_recent_context_info_with_exchanges() {
        let info = RecentContextInfo {
            total_messages: 10,
            exchanges: vec![(
                RecentMessage {
                    role_label: "👤 User".to_string(),
                    content: "What is Rust?".to_string(),
                },
                Some(RecentMessage {
                    role_label: "🤖 Assistant".to_string(),
                    content: "Rust is a systems programming language...".to_string(),
                }),
            )],
        };
        let output = info.format_context_summary();
        assert!(output.contains("Recent context"));
        assert!(output.contains("10 messages"));
        assert!(output.contains("👤 User"));
        assert!(output.contains("🤖 Assistant"));
        assert!(output.contains("What is Rust?"));
    }

    #[test]
    fn test_recent_context_info_user_only() {
        let info = RecentContextInfo {
            total_messages: 3,
            exchanges: vec![(
                RecentMessage {
                    role_label: "👤 User".to_string(),
                    content: "Hello".to_string(),
                },
                None,
            )],
        };
        let output = info.format_context_summary();
        assert!(output.contains("👤 User"));
        assert!(output.contains("Hello"));
        // Should NOT contain assistant content when None
        assert!(!output.contains("🤖 Assistant"));
    }

    #[test]
    fn test_recent_context_info_strips_thinking_tags() {
        // Verify that thinking tags are not shown in context display
        use crate::chat::strip_thinking_tags;

        // HTML thinking tags should be stripped
        let input = "<thinking>Let me think about this...</thinking>\n\nThe answer is 42";
        let cleaned = strip_thinking_tags(input);
        assert!(!cleaned.contains("<thinking>"));
        assert!(!cleaned.contains("</thinking>"));
        assert!(cleaned.contains("The answer is 42"));

        // When truncated, thinking content should not appear
        let truncated = truncate_str(&cleaned, MAX_CONTEXT_LINE_LENGTH);
        assert!(!truncated.contains("Let me think"));
        assert!(truncated.contains("The answer is 42"));

        // Unicode thinking tags should also be stripped
        let unicode_input = "\u{6beb}Internal reasoning\u{6beb}\n\nFinal response";
        let unicode_cleaned = strip_thinking_tags(unicode_input);
        assert!(!unicode_cleaned.contains("Internal reasoning"));
        assert!(unicode_cleaned.contains("Final response"));
    }

    #[test]
    fn test_recent_context_info_newlines_collapsed() {
        // Verify that when newlines are replaced with spaces before creating
        // RecentMessage (as done in show_recent_context), each message
        // displays on a single line in the context summary.
        let info = RecentContextInfo {
            total_messages: 4,
            exchanges: vec![(
                RecentMessage {
                    role_label: "👤 User".to_string(),
                    // Content already has newlines replaced with spaces
                    // (this happens in show_recent_context before truncate_str)
                    content: "Hello world how are you?".to_string(),
                },
                Some(RecentMessage {
                    role_label: "🤖 Assistant".to_string(),
                    content: "I am fine thanks!".to_string(),
                }),
            )],
        };
        let output = info.format_context_summary();

        // No line should contain a bare newline inside the message content
        // Each message should be on a single line after its role label
        for line in output.lines() {
            // Lines with role labels should not contain embedded \n
            // (they are already single lines since \n was replaced before)
            // We just verify the output contains the content as expected
            if line.contains("👤") {
                assert!(line.contains("Hello world how are you?"));
                assert!(!line.contains("\n"));
            }
            if line.contains("🤖") {
                assert!(line.contains("I am fine thanks!"));
                assert!(!line.contains("\n"));
            }
        }
    }
}
