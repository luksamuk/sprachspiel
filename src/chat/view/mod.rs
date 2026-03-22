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

#![allow(dead_code)]

// ANSI color codes for banner styling
mod colors {
    pub const CYAN: &str = "\x1B[36m";
    pub const YELLOW: &str = "\x1B[33m";
    pub const BOLD: &str = "\x1B[1m";
    pub const DIM: &str = "\x1B[2m";
    pub const RESET: &str = "\x1B[0m";
    pub const BOLD_CYAN: &str = "\x1B[1;36m";
    pub const BOLD_YELLOW: &str = "\x1B[1;33m";
}

/// ASCII art logo using toilet "future" font (pre-rendered)
const BANNER_LOGO: &str = "\
\x1B[1;34;94m┏━┓┏━┓╻┏\x1B[0m    \x1B[1;34;94m┏━┓╻\x1B[0m
\x1B[1;34;94m┣━┫┗━┓┣┻\x1B[0;34m┓╺━╸┣━┫┃\x1B[0m
\x1B[0;34m╹\x1B[0m \x1B[0;34m╹┗━┛╹\x1B[0m \x1B[0;34m╹\x1B[0m   \x1B[0;34m╹\x1B[0m \x1B[0;34m╹╹\x1B[0m";

/// Extended Mind ASCII art - generated from thumb4.png via jp2a
/// Brain (cyan) with extensions (orange/brown) representing tools/memory/Zettelkasten
/// Generated: magick thumb4.png -crop 900x600+300+200 -resize 120x85 | jp2a --width=40 --colors
const EXTENDED_MIND_ART: [&str; 12] = [
"\x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m    \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m      \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m \x1B[38;2;4;2;1m \x1B[38;2;16;8;1m \x1B[38;2;54;30;4m.\x1B[38;2;139;111;60mc\x1B[38;2;190;151;66md\x1B[38;2;176;133;40mo\x1B[38;2;195;170;83mx\x1B[38;2;139;104;23m:\x1B[0m",
"\x1B[38;2;5;3;1m \x1B[38;2;28;14;1m \x1B[38;2;70;47;14m.\x1B[38;2;59;37;6m.\x1B[38;2;17;8;1m \x1B[38;2;3;2;1m \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m          \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;4;3;1m \x1B[38;2;14;7;1m \x1B[38;2;47;28;5m.\x1B[38;2;113;90;49m;\x1B[38;2;153;119;58ml\x1B[38;2;152;119;58mc\x1B[38;2;106;83;41m,\x1B[38;2;42;23;2m \x1B[38;2;18;9;1m \x1B[38;2;17;8;1m \x1B[38;2;10;5;1m\x1B[0m",
"\x1B[38;2;24;12;1m \x1B[38;2;167;127;26ml\x1B[38;2;253;247;168mW\x1B[38;2;246;234;136mN\x1B[38;2;153;108;26mc\x1B[38;2;85;66;35m'\x1B[38;2;43;26;7m \x1B[38;2;18;9;1m \x1B[38;2;8;4;1m \x1B[38;2;3;2;1m \x1B[38;2;2;2;1m    \x1B[38;2;2;2;2m \x1B[38;2;1;5;5m \x1B[38;2;1;12;13m \x1B[38;2;6;32;33m \x1B[38;2;57;81;83m'\x1B[38;2;85;110;111m:\x1B[38;2;84;114;115m:\x1B[38;2;41;89;92m'\x1B[38;2;66;108;112m;\x1B[38;2;64;105;107m;\x1B[38;2;59;89;91m,\x1B[38;2;64;86;87m,\x1B[38;2;34;56;58m.\x1B[38;2;2;16;15m \x1B[38;2;15;11;3m \x1B[38;2;76;58;29m.\x1B[38;2;125;99;48m:\x1B[38;2;140;106;50m:\x1B[38;2;130;101;50m:\x1B[38;2;84;63;30m'\x1B[38;2;31;15;1m \x1B[38;2;10;5;1m \x1B[38;2;3;2;1m \x1B[38;2;2;1;1m    \x1B[0m",
"\x1B[38;2;5;3;1m \x1B[38;2;28;15;1m \x1B[38;2;71;54;9m.\x1B[38;2;64;45;5m.\x1B[38;2;54;33;8m.\x1B[38;2;90;72;38m'\x1B[38;2;119;93;49m;\x1B[38;2;129;97;48m:\x1B[38;2;132;99;47m:\x1B[38;2;125;96;47m:\x1B[38;2;105;84;45m,\x1B[38;2;62;47;21m.\x1B[38;2;12;19;13m \x1B[38;2;38;74;76m.\x1B[38;2;123;158;160md\x1B[38;2;147;191;194mk\x1B[38;2;145;201;206mO\x1B[38;2;181;216;219mK\x1B[38;2;175;217;220mK\x1B[38;2;189;223;226mK\x1B[38;2;128;189;195mk\x1B[38;2;187;222;226mK\x1B[38;2;164;211;216m0\x1B[38;2;128;188;193mk\x1B[38;2;193;226;229mX\x1B[38;2;174;219;222mK\x1B[38;2;138;184;186mk\x1B[38;2;102;143;130ml\x1B[38;2;145;163;128md\x1B[38;2;48;53;33m.\x1B[38;2;13;10;4m \x1B[38;2;4;3;1m \x1B[38;2;2;1;1m      \x1B[38;2;2;1;1m \x1B[38;2;2;1;1m \x1B[0m",
"\x1B[38;2;2;1;1m   \x1B[38;2;2;1;1m \x1B[38;2;3;2;1m \x1B[38;2;6;3;1m \x1B[38;2;14;8;1m \x1B[38;2;32;16;1m \x1B[38;2;70;52;21m.\x1B[38;2;105;117;74m:\x1B[38;2;123;161;151md\x1B[38;2;190;227;229mX\x1B[38;2;215;239;241mN\x1B[38;2;71;154;161ml\x1B[38;2;204;234;237mX\x1B[38;2;235;249;250mW\x1B[38;2;200;231;234mX\x1B[38;2;169;213;217m0\x1B[38;2;184;218;220mK\x1B[38;2;201;230;232mX\x1B[38;2;220;242;243mN\x1B[38;2;200;228;231mX\x1B[38;2;211;238;241mN\x1B[38;2;207;234;236mN\x1B[38;2;148;200;205mO\x1B[38;2;196;229;232mX\x1B[38;2;188;226;230mX\x1B[38;2;116;179;185mx\x1B[38;2;15;54;56m.\x1B[38;2;1;7;7m \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m   \x1B[38;2;2;1;1m    \x1B[0m",
"\x1B[38;2;2;2;1m  \x1B[38;2;2;1;1m \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m  \x1B[38;2;2;2;1m    \x1B[38;2;2;14;14m \x1B[38;2;88;140;143ml\x1B[38;2;158;210;215m0\x1B[38;2;225;244;246mW\x1B[38;2;80;164;171mo\x1B[38;2;220;238;239mN\x1B[38;2;204;233;236mX\x1B[38;2;202;228;231mX\x1B[38;2;121;185;191mx\x1B[38;2;184;219;222mK\x1B[38;2;197;227;229mX\x1B[38;2;158;207;212m0\x1B[38;2;216;239;241mN\x1B[38;2;213;234;236mN\x1B[38;2;115;181;187mx\x1B[38;2;183;218;221mK\x1B[38;2;148;199;203mO\x1B[38;2;168;208;211m0\x1B[38;2;189;224;227mK\x1B[38;2;165;209;214m0\x1B[38;2;141;195;199mO\x1B[38;2;1;26;26m \x1B[38;2;2;3;2m   \x1B[38;2;2;1;1m    \x1B[38;2;2;2;1m\x1B[0m",
"\x1B[38;2;2;2;1m       \x1B[38;2;2;2;1m \x1B[38;2;1;5;5m \x1B[38;2;20;55;56m.\x1B[38;2;141;178;181mx\x1B[38;2;205;234;236mX\x1B[38;2;77;156;162ml\x1B[38;2;219;241;243mN\x1B[38;2;243;251;252mW\x1B[38;2;183;217;220mK\x1B[38;2;214;237;239mN\x1B[38;2;210;235;237mN\x1B[38;2;214;236;238mN\x1B[38;2;139;195;200mk\x1B[38;2;150;200;204mO\x1B[38;2;193;222;224mK\x1B[38;2;202;231;234mX\x1B[38;2;171;213;216m0\x1B[38;2;192;224;227mX\x1B[38;2;186;221;224mK\x1B[38;2;176;219;222mK\x1B[38;2;228;246;247mW\x1B[38;2;80;132;135mc\x1B[38;2;1;16;16m \x1B[38;2;2;2;2m   \x1B[38;2;2;1;1m     \x1B[0m",
"\x1B[38;2;2;2;1m      \x1B[38;2;4;3;1m \x1B[38;2;12;7;1m \x1B[38;2;36;20;3m \x1B[38;2;85;68;35m'\x1B[38;2;120;98;48m:\x1B[38;2;93;76;33m,\x1B[38;2;25;40;34m.\x1B[38;2;13;41;42m.\x1B[38;2;53;86;88m,\x1B[38;2;107;133;135ml\x1B[38;2;123;148;150mo\x1B[38;2;115;143;144mo\x1B[38;2;45;100;103m,\x1B[38;2;113;172;177md\x1B[38;2;170;215;219m0\x1B[38;2;249;254;254mM\x1B[38;2;202;232;235mX\x1B[38;2;184;223;226mK\x1B[38;2;155;204;207mO\x1B[38;2;187;222;225mK\x1B[38;2;196;226;229mX\x1B[38;2;185;220;223mK\x1B[38;2;65;127;127m:\x1B[38;2;64;64;38m.\x1B[38;2;59;45;21m.\x1B[38;2;26;13;1m \x1B[38;2;10;5;1m \x1B[38;2;4;3;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m    \x1B[38;2;2;2;1m\x1B[0m",
"\x1B[38;2;2;2;1m   \x1B[38;2;3;2;1m \x1B[38;2;8;4;1m \x1B[38;2;24;12;1m \x1B[38;2;71;49;18m.\x1B[38;2;128;104;55m:\x1B[38;2;152;117;57mc\x1B[38;2;147;115;58mc\x1B[38;2;108;85;43m;\x1B[38;2;49;28;4m.\x1B[38;2;14;7;1m \x1B[38;2;3;3;1m  \x1B[38;2;2;2;2m \x1B[38;2;2;3;2m \x1B[38;2;2;5;5m \x1B[38;2;1;16;17m \x1B[38;2;22;52;53m.\x1B[38;2;53;78;80m'\x1B[38;2;52;79;81m'\x1B[38;2;21;76;80m.\x1B[38;2;49;155;162ml\x1B[38;2;101;159;164mo\x1B[38;2;94;127;129mc\x1B[38;2;51;82;84m'\x1B[38;2;8;20;16m \x1B[38;2;55;38;15m.\x1B[38;2;119;95;52m;\x1B[38;2;146;111;55mc\x1B[38;2;148;113;56mc\x1B[38;2;126;100;54m:\x1B[38;2;73;51;21m.\x1B[38;2;28;13;1m \x1B[38;2;10;5;1m \x1B[38;2;3;2;1m \x1B[0m",
"\x1B[38;2;94;75;28m,\x1B[38;2;104;85;45m,\x1B[38;2;80;53;10m.\x1B[38;2;117;95;47m;\x1B[38;2;137;104;50m:\x1B[38;2;137;104;50m:\x1B[38;2;117;94;48m;\x1B[38;2;66;46;17m.\x1B[38;2;23;11;1m \x1B[38;2;8;4;1m \x1B[38;2;3;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m  \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m   \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;2;4;4m \x1B[38;2;2;12;13m \x1B[38;2;5;21;22m \x1B[38;2;1;5;5m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m \x1B[38;2;3;2;1m \x1B[38;2;9;5;1m \x1B[38;2;25;12;1m \x1B[38;2;66;45;17m.\x1B[38;2;112;89;47m;\x1B[38;2;134;101;49m:\x1B[38;2;136;102;48m:\x1B[38;2;125;97;41m:\x1B[38;2;111;79;30m,\x1B[0m",
"\x1B[38;2;235;221;142mX\x1B[38;2;240;230;161mX\x1B[38;2;176;135;39mo\x1B[38;2;70;45;11m.\x1B[38;2;20;10;1m \x1B[38;2;7;4;1m \x1B[38;2;3;2;1m \x1B[38;2;2;2;1m     \x1B[38;2;2;1;1m    \x1B[38;2;2;2;1m  \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m  \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m \x1B[38;2;2;1;1m    \x1B[38;2;2;1;1m \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m \x1B[38;2;2;2;1m \x1B[38;2;5;3;1m \x1B[38;2;14;7;1m \x1B[38;2;49;26;2m.\x1B[38;2;150;115;61mc\x1B[0m",
"\x1B[38;2;35;18;1m \x1B[38;2;40;22;1m \x1B[38;2;17;8;1m \x1B[38;2;4;2;1m \x1B[38;2;2;2;1m         \x1B[38;2;2;1;1m    \x1B[38;2;2;2;1m  \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m  \x1B[38;2;2;1;1m   \x1B[38;2;2;1;1m\x1B[38;2;2;1;1m \x1B[38;2;2;1;1m\x1B[38;2;2;1;1m     \x1B[38;2;2;1;1m \x1B[38;2;2;2;1m \x1B[0m",
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
    pub sandbox_status: String,
    pub project: String,
    pub session_name: String,
    pub is_anonymous: bool,
}

impl WelcomeInfo {
    /// Format the welcome banner with ASCII art and session info
    pub fn to_boxed_string(&self) -> String {
        let mut output = String::new();
        output.push('\n');

        output.push_str(BANNER_LOGO);
        output.push('\n');
        output.push('\n');

        // Simple dim line separator
        output.push_str(&format!("{}{}\n", colors::DIM, "─".repeat(80)));
        output.push('\n');

        let session_lines = self.format_session_lines();

        // Calculate visual width of each art line (strip ANSI codes)
        let art_visual_widths: Vec<usize> = EXTENDED_MIND_ART
            .iter()
            .map(|line| strip_ansi_width(line))
            .collect();

        // Find max width to align session info
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
    fn format_session_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(format!(
            "{}Model:{} {}{}{}",
            colors::BOLD_CYAN,
            colors::RESET,
            colors::DIM,
            truncate_str(&self.model_id, 35),
            colors::RESET
        ));

        if self.think_enabled {
            lines.push(format!(
                "{}Think:{} {}enabled{}",
                colors::BOLD_CYAN,
                colors::RESET,
                colors::DIM,
                colors::RESET
            ));
        }

        let tools_status = if self.tools_enabled {
            "enabled"
        } else {
            "disabled"
        };
        lines.push(format!(
            "{}Tools:{} {}{}{}",
            colors::BOLD_CYAN,
            colors::RESET,
            colors::DIM,
            tools_status,
            colors::RESET
        ));

        lines.push(format!(
            "{}Sandbox:{} {}{}{}",
            colors::BOLD_CYAN,
            colors::RESET,
            colors::DIM,
            truncate_str(&self.sandbox_status, 28),
            colors::RESET
        ));

        lines.push(format!(
            "{}Project:{} {}{}{}",
            colors::BOLD_CYAN,
            colors::RESET,
            colors::DIM,
            truncate_str(&self.project, 34),
            colors::RESET
        ));

        let session_display = if self.is_anonymous {
            "anonymous".to_string()
        } else {
            self.session_name.clone()
        };
        lines.push(format!(
            "{}Session:{} {}{}{}",
            colors::BOLD_CYAN,
            colors::RESET,
            colors::DIM,
            truncate_str(&session_display, 34),
            colors::RESET
        ));

        lines
    }
}

/// Truncate a string to a maximum length, adding ellipsis if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Calculate visual width of a string, stripping ANSI escape codes
fn strip_ansi_width(s: &str) -> usize {
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
            model_id: "llama3.1".to_string(),
            tools_enabled: true,
            think_enabled: true,
            sandbox_status: "enabled (landlock)".to_string(),
            project: "my-project".to_string(),
            session_name: "default".to_string(),
            is_anonymous: false,
        };

        let output = info.to_boxed_string();
        assert!(output.contains("llama3.1"));
        assert!(output.contains("Project:"));
        assert!(output.contains("Session:"));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a very long string", 10), "this is...");
    }
}
