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

/// Number of lines in the status bar (divisor, content, divisor)
/// Used by ANSI clear codes in repl.rs to remove status bar before user input
pub const STATUS_BAR_LINES: usize = 3;

// ANSI color codes for banner styling
mod colors {
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
const BANNER_LOGO: &str = "\
\x1B[1;34;94m┏━┓┏━┓╻┏\x1B[0m    \x1B[1;34;94m┏━┓╻\x1B[0m
\x1B[1;34;94m┣━┫┗━┓┣┻\x1B[0;34m┓╺━╸┣━┫┃\x1B[0m
\x1B[0;34m╹\x1B[0m \x1B[0;34m╹┗━┛╹\x1B[0m \x1B[0;34m╹\x1B[0m   \x1B[0;34m╹\x1B[0m \x1B[0;34m╹╹\x1B[0m";

/// Neuron braille art - generated from neuronio3.png via braille_art.py
/// Brain with extensions representing tools/memory/Zettelkasten
/// Generated: python3 braille_art.py neuronio3.png -w 45 --color
const NEURON_BRAILLE_ART: [&str; 30] = [
    "⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;96;171;177m⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀\x1B[38;2;127;255;255m⢀⠀⠀⠀⠀\x1B[38;2;78;154;157m⢀\x1B[38;2;110;193;199m⡆\x1B[38;2;127;255;255m⠸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;89;173;186m⠳\x1B[38;2;121;192;199m⣼⠀\x1B[38;2;122;196;201m⣸⠀⠀\x1B[38;2;90;147;157m⠠⠀\x1B[38;2;88;165;174m⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;188;228;232m⠸\x1B[38;2;165;218;222m⡆\x1B[38;2;140;204;209m⡏⠀⠀⠀\x1B[38;2;123;197;203m⣶⠀⠀⠀⠀\x1B[38;2;95;173;181m⡠⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀\x1B[38;2;89;155;160m⠸\x1B[38;2;90;158;163m⡀⠀⠀⠀⠀⠀\x1B[38;2;191;233;236m⢻\x1B[38;2;213;251;253m⡇⠀⠀⠀\x1B[38;2;123;181;185m⣿⠀⠀\x1B[38;2;136;212;216m⢠\x1B[38;2;104;194;200m⡴\x1B[38;2;101;187;195m⠁\x1B[38;2;85;170;170m⢀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀\x1B[38;2;137;209;215m⡇⠀⠀⠀⠀⠀\x1B[38;2;140;200;204m⢸\x1B[38;2;201;229;231m⣧⠀⠀⠀\x1B[38;2;135;201;205m⣿\x1B[38;2;192;246;247m⢀\x1B[38;2;180;238;240m⡴\x1B[38;2;182;242;244m⠋⠀⠀\x1B[38;2;86;165;173m⠠⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "\x1B[38;2;191;255;255m⠰\x1B[38;2;87;170;182m⠐\x1B[38;2;87;171;182m⣄\x1B[38;2;147;203;209m⢹\x1B[38;2;142;199;202m⡄⠀⠀⠀⠀\x1B[38;2;74;151;157m⠈\x1B[38;2;230;250;251m⣿\x1B[38;2;135;209;215m⡄⠀\x1B[38;2;156;212;216m⣰\x1B[38;2;221;248;250m⣿\x1B[38;2;184;236;238m⠏⠀⠀⠀⠀\x1B[38;2;92;169;177m⡆\x1B[38;2;255;255;255m⠈⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀\x1B[38;2;139;227;235m⠈\x1B[38;2;133;206;213m⠳\x1B[38;2;201;237;239m⣷\x1B[38;2;163;212;219m⡄⠀⠀⠀\x1B[38;2;115;193;200m⢠\x1B[38;2;242;253;253m⣿\x1B[38;2;231;250;251m⣿\x1B[38;2;202;236;238m⣿\x1B[38;2;238;252;252m⣿\x1B[38;2;214;241;243m⡏⠀⠀⠀⠀\x1B[38;2;146;211;215m⣼\x1B[38;2;123;209;217m⠖\x1B[38;2;133;223;230m⠒⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀\x1B[38;2;91;174;184m⠈\x1B[38;2;196;236;239m⢻\x1B[38;2;221;249;251m⣦\x1B[38;2;172;226;229m⣄\x1B[38;2;159;223;228m⣠\x1B[38;2;214;243;245m⣾\x1B[38;2;241;253;252m⣿\x1B[38;2;231;250;250m⣿\x1B[38;2;230;250;250m⣿\x1B[38;2;240;252;252m⣿\x1B[38;2;204;236;238m⣿\x1B[38;2;168;231;236m⡀⠀\x1B[38;2;154;216;221m⣠\x1B[38;2;176;226;230m⡾\x1B[38;2;169;226;229m⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀\x1B[38;2;80;148;157m⢀⠀⠀⠀\x1B[38;2;194;238;242m⣹\x1B[38;2;239;252;252m⣿\x1B[38;2;240;253;252m⣿\x1B[38;2;241;252;252m⣿\x1B[38;2;203;237;240m⡿\x1B[38;2;176;231;234m⣡\x1B[38;2;157;220;225m⣬\x1B[38;2;215;245;246m⢻\x1B[38;2;241;252;253m⣿\x1B[38;2;235;253;253m⣿\x1B[38;2;227;251;252m⣿\x1B[38;2;213;248;249m⣿\x1B[38;2;183;236;239m⣤\x1B[38;2;163;218;222m⣄\x1B[38;2;102;200;210m⡠\x1B[38;2;95;175;184m⠖\x1B[38;2;255;255;255m⠐⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "\x1B[38;2;150;238;246m⢀⠀\x1B[38;2;115;195;209m⠈\x1B[38;2;135;208;217m⢲\x1B[38;2;197;244;245m⡴\x1B[38;2;181;229;232m⠟\x1B[38;2;208;243;245m⠛\x1B[38;2;205;240;243m⠻\x1B[38;2;229;247;248m⣿\x1B[38;2;245;253;253m⣿\x1B[38;2;154;221;226m⡇\x1B[38;2;236;252;252m⣿\x1B[38;2;242;253;253m⣿\x1B[38;2;101;188;193m⢾\x1B[38;2;245;252;252m⣿\x1B[38;2;221;245;246m⣿\x1B[38;2;169;227;232m⠋⠀⠀\x1B[38;2;179;245;250m⠈\x1B[38;2;136;223;236m⠢\x1B[38;2;118;194;208m⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀\x1B[38;2;127;205;212m⠙\x1B[38;2;121;196;204m⡻\x1B[38;2;169;226;231m⠋⠀⠀⠀⠀\x1B[38;2;136;203;207m⢸\x1B[38;2;244;253;252m⣿\x1B[38;2;198;237;239m⣧\x1B[38;2;191;237;239m⠻\x1B[38;2;205;241;244m⠟\x1B[38;2;181;227;229m⣼\x1B[38;2;244;253;253m⣿\x1B[38;2;197;238;241m⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀\x1B[38;2;76;155;163m⠁⠀⠀⠀⠀⠀\x1B[38;2;98;178;184m⢸\x1B[38;2;244;252;253m⣿\x1B[38;2;241;253;253m⣿\x1B[38;2;223;248;249m⣷\x1B[38;2;220;247;248m⣾\x1B[38;2;241;253;253m⣿\x1B[38;2;242;253;253m⣿\x1B[38;2;195;237;240m⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;114;192;199m⢀\x1B[38;2;202;235;237m⣾\x1B[38;2;230;251;251m⡿\x1B[38;2;212;241;242m⣿\x1B[38;2;239;252;252m⣿\x1B[38;2;234;253;253m⣿\x1B[38;2;216;243;245m⣿\x1B[38;2;236;252;252m⣿\x1B[38;2;195;232;234m⣿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀\x1B[38;2;117;200;209m⠤\x1B[38;2;140;225;232m⠤\x1B[38;2;174;227;231m⣤\x1B[38;2;170;228;232m⡴\x1B[38;2;200;240;241m⢿\x1B[38;2;180;227;230m⠟⠀\x1B[38;2;119;198;205m⠈\x1B[38;2;223;248;249m⣿\x1B[38;2;149;214;220m⠃⠀\x1B[38;2;115;191;197m⠙\x1B[38;2;221;245;246m⢿\x1B[38;2;181;226;228m⣇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀\x1B[38;2;107;202;215m⢀\x1B[38;2;94;178;187m⠜\x1B[38;2;93;178;189m⠁⠀\x1B[38;2;149;210;214m⡟⠀⠀⠀\x1B[38;2;160;221;225m⡟⠀⠀⠀\x1B[38;2;80;164;169m⠈\x1B[38;2;187;219;214m⠛\x1B[38;2;236;220;137m⣴\x1B[38;2;232;196;84m⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀\x1B[38;2;172;231;234m⢰\x1B[38;2;119;180;184m⠃⠀\x1B[38;2;170;170;170m⠐⠀\x1B[38;2;193;245;247m⡇⠀⠀⠀⠀\x1B[38;2;169;136;49m⠘\x1B[38;2;246;237;183m⣿\x1B[38;2;251;246;194m⣷\x1B[38;2;226;194;91m⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀\x1B[38;2;99;180;188m⣠\x1B[38;2;99;180;185m⢻⠀⠀⠀\x1B[38;2;96;173;179m⢸\x1B[38;2;103;182;191m⢡⠀⠀⠀⠀⠀\x1B[38;2;217;181;83m⠘\x1B[38;2;251;246;196m⢿\x1B[38;2;250;245;198m⣿\x1B[38;2;223;196;100m⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;232;218;144m⣴\x1B[38;2;238;227;157m⣦⠀⠀⠀\x1B[0m",
    "⠀⠀⠀\x1B[38;2;85;170;170m⠈⠀\x1B[38;2;131;219;229m⠁⠀⠀\x1B[38;2;170;255;255m⠰\x1B[38;2;82;172;184m⠠\x1B[38;2;114;192;201m⠃⠀\x1B[38;2;0;255;255m⡀⠀⠀⠀⠀⠀\x1B[38;2;213;176;66m⠈\x1B[38;2;248;239;177m⠻\x1B[38;2;210;170;77m⢣\x1B[38;2;240;225;155m⣶\x1B[38;2;243;221;102m⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;228;209;123m⣿\x1B[38;2;238;226;156m⠟⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;219;192;108m⠸\x1B[38;2;248;242;195m⣿\x1B[38;2;251;246;199m⣿\x1B[38;2;246;233;156m⣦\x1B[38;2;236;209;83m⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;218;200;124m⣸\x1B[38;2;215;191;110m⠃⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;232;205;81m⠈\x1B[38;2;249;240;172m⠻\x1B[38;2;251;247;202m⣿\x1B[38;2;237;222;153m⡗\x1B[38;2;233;214;136m⣤\x1B[38;2;238;219;120m⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;225;210;140m⣴\x1B[38;2;232;220;154m⠃⠀⠀\x1B[38;2;241;220;119m⢀\x1B[38;2;243;228;136m⣄⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;237;217;77m⠈\x1B[38;2;199;155;46m⠁\x1B[38;2;250;243;182m⢿\x1B[38;2;253;251;218m⣿\x1B[38;2;250;244;191m⣿\x1B[38;2;248;241;177m⣶\x1B[38;2;190;133;40m⡀\x1B[38;2;234;218;124m⣄\x1B[38;2;241;218;84m⣀\x1B[38;2;206;161;33m⣀\x1B[38;2;187;133;15m⡀⠀\x1B[38;2;179;138;49m⢀\x1B[38;2;248;239;174m⣠\x1B[38;2;235;222;167m⣾\x1B[38;2;231;213;142m⣧\x1B[38;2;232;221;152m⣤\x1B[38;2;237;227;155m⣤\x1B[38;2;213;197;128m⣴\x1B[38;2;250;242;173m⢾\x1B[38;2;252;248;197m⣿⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;231;206;103m⠙\x1B[38;2;245;233;162m⠻\x1B[38;2;252;246;180m⠿\x1B[38;2;190;149;63m⢸\x1B[38;2;253;252;219m⣿\x1B[38;2;254;252;218m⣿\x1B[38;2;253;250;210m⣿\x1B[38;2;235;216;160m⣿\x1B[38;2;235;224;168m⢸\x1B[38;2;247;236;174m⡟\x1B[38;2;222;196;119m⠋⠀⠀⠀⠀⠀\x1B[38;2;181;136;41m⠈\x1B[38;2;245;235;123m⠁⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;223;187;60m⠉\x1B[38;2;231;210;108m⠙\x1B[38;2;229;207;116m⠛\x1B[38;2;223;199;115m⠋\x1B[38;2;219;189;91m⠈\x1B[38;2;230;213;151m⠻\x1B[38;2;226;208;148m⣦⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;231;215;162m⠘\x1B[38;2;242;230;172m⣷\x1B[38;2;247;239;175m⣄\x1B[38;2;215;191;108m⡀\x1B[38;2;217;184;74m⢀\x1B[38;2;228;206;107m⣄⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;216;193;126m⢹\x1B[38;2;198;167;99m⡌\x1B[38;2;239;230;170m⠙\x1B[38;2;249;239;165m⢻\x1B[38;2;253;250;201m⣿⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;216;195;129m⣇⠀\x1B[38;2;189;152;46m⠈\x1B[38;2;225;206;100m⠉⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;223;204;130m⣽\x1B[38;2;249;239;154m⡄⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;241;230;164m⣿\x1B[38;2;233;220;149m⡏⠀⠀⠀⠀⠀\x1B[0m",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[0m",
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
    pub vision_enabled: bool,
    pub sandbox_status: String,
    pub project: String,
    pub session_name: String,
    pub is_anonymous: bool,
    pub version: String,
    pub ollama_url: String,
    pub embed_model: String,
    pub db_stats: String,
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

        let art_visual_widths: Vec<usize> = NEURON_BRAILLE_ART
            .iter()
            .map(|line| strip_ansi_width(line))
            .collect();

        let max_art_width = art_visual_widths.iter().max().copied().unwrap_or(0);

        for (i, art_line) in NEURON_BRAILLE_ART.iter().enumerate() {
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

        let bc = colors::BOLD_CYAN;
        let d = colors::DIM;
        let r = colors::RESET;

        lines.push(format!(
            "{}Model:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.model_id, 30),
            r
        ));

        if self.think_enabled {
            lines.push(format!("{}Think:{} {}enabled{}", bc, r, d, r));
        }

        let tools_status = if self.tools_enabled {
            "enabled"
        } else {
            "disabled"
        };
        lines.push(format!("{}Tools:{} {}{}{}", bc, r, d, tools_status, r));

        if self.vision_enabled {
            lines.push(format!("{}Vision:{} {}enabled{}", bc, r, d, r));
        }

        lines.push(format!(
            "{}Sandbox:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.sandbox_status, 28),
            r
        ));

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

        lines.push(format!("{}Version:{} {}{}{}", bc, r, d, self.version, r));

        lines.push(format!(
            "{}Ollama:{} {}{}{}",
            bc,
            r,
            d,
            truncate_str(&self.ollama_url, 30),
            r
        ));

        if !self.embed_model.is_empty() {
            lines.push(format!(
                "{}Embed:{} {}{}{}",
                bc,
                r,
                d,
                truncate_str(&self.embed_model, 30),
                r
            ));
        }

        if !self.db_stats.is_empty() {
            lines.push(format!("{}DB:{} {}{}{}", bc, r, d, self.db_stats, r));
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

/// Truncate a string to a maximum length, adding ellipsis if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Truncate a string to a maximum visual width (Unicode-aware, ANSI-aware)
/// Each Unicode character counts as 1 visual column, even multi-byte chars.
/// ANSI escape codes are preserved but not counted.
fn truncate_visual(s: &str, max_width: usize) -> String {
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
            model_id: "qwen3.5:4b".to_string(),
            tools_enabled: true,
            think_enabled: true,
            vision_enabled: false,
            sandbox_status: "enabled (landlock)".to_string(),
            project: "my-project".to_string(),
            session_name: "default".to_string(),
            is_anonymous: false,
            version: "0.39.5".to_string(),
            ollama_url: "localhost:11434".to_string(),
            embed_model: "nomic-embed-text".to_string(),
            db_stats: "3 facts, 2 notes, 1 doc".to_string(),
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
}
