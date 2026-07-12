//! Status bar component — context usage, model name, and spinner
//!
//! Renders the status bar at the bottom of the chat area (above input),
//! showing model name, context usage, progress bar, and thinking/tools indicators.
//!
//! During LLM processing, the model name is replaced with an animated spinner.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::chat::app::{EmbeddingPhase, EmbeddingProgress};

use super::super::styles;

/// Status bar state for rendering
#[derive(Debug, Clone)]
pub struct StatusBarState {
    /// Model name (e.g., "llama3.1")
    pub model_name: String,
    /// Used tokens in context
    pub used_tokens: usize,
    /// Maximum context tokens
    pub max_tokens: usize,
    /// Context usage percentage (0-100)
    pub percent: u8,
    /// Whether thinking mode is active
    pub think_enabled: bool,
    /// Whether tools are active
    pub tools_enabled: bool,
    /// Whether style rendering is enabled (mermaid diagrams, LaTeX formulas, syntax
    /// highlighting, box-drawing tables). Shown as 🎨 (on) or 📄 (off)
    /// in the status bar indicator row.
    pub style_enabled: bool,
    /// Spinner character (if animating, e.g., "⠋")
    pub spinner: Option<String>,
    /// Status label (e.g., "Thinking...", "Running tool...")
    pub status_label: Option<String>,
    /// Embedding progress: current phase and counts when embeddings are being generated
    pub embedding_progress: Option<EmbeddingProgress>,
    /// Optional transient right-aligned overlay (e.g., provider retry warning).
    ///
    /// Rendered in red on the right side of the status bar.
    pub overlay: Option<String>,
}

impl StatusBarState {
    /// Create a new idle status bar state
    pub fn new(
        model_name: String,
        used_tokens: usize,
        max_tokens: usize,
        percent: u8,
        think_enabled: bool,
        tools_enabled: bool,
    ) -> Self {
        Self {
            model_name,
            used_tokens,
            max_tokens,
            percent,
            think_enabled,
            tools_enabled,
            style_enabled: true,
            spinner: None,
            status_label: None,
            embedding_progress: None,
            overlay: None,
        }
    }

    /// Format token count as human-readable (e.g., "47.2K")
    pub fn format_tokens(tokens: usize) -> String {
        if tokens >= 1_000_000 {
            format!("{:.1}M", tokens as f64 / 1_000_000.0)
        } else if tokens >= 1_000 {
            format!("{:.1}K", tokens as f64 / 1_000.0)
        } else {
            tokens.to_string()
        }
    }
}

/// Render the status bar
///
/// Layout:
/// ```text
/// ────────────────────────────────────────────────
///  ⠋ Thinking... │ 47.2K/128K ██░░ 37% │ 🧠🔧
/// ────────────────────────────────────────────────
/// ```
pub fn render(f: &mut Frame, area: Rect, state: &StatusBarState) {
    let mut spans: Vec<Span> = Vec::new();

    // Left section: model name or spinner+label
    if let (Some(spinner), Some(label)) = (&state.spinner, &state.status_label) {
        // Spinner character (green bold), space, label text (yellow bold)
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            spinner.clone(),
            Style::default()
                .fg(styles::GREEN)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {}", label),
            Style::default()
                .fg(styles::YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        // Truncate model name to 20 wide characters
        let model_display = if state.model_name.chars().count() > 20 {
            let truncated: String = state.model_name.chars().take(19).collect();
            format!("{}…", truncated)
        } else {
            state.model_name.clone()
        };
        spans.push(Span::styled(
            model_display,
            Style::default().add_modifier(Modifier::BOLD),
        ));
    }

    // Separator
    spans.push(Span::raw(" │ "));

    // Context section: tokens
    let used_str = StatusBarState::format_tokens(state.used_tokens);
    let max_str = StatusBarState::format_tokens(state.max_tokens);
    spans.push(Span::styled(
        format!("{}/{}", used_str, max_str),
        Style::default(),
    ));

    // Progress bar
    spans.push(Span::raw(" "));
    let bar_width = 12;
    let filled = (state.percent as usize * bar_width) / 100;
    let empty = bar_width.saturating_sub(filled);
    let bar_color = styles::progress_color(state.percent);
    spans.push(Span::styled(
        "█".repeat(filled),
        Style::default().fg(bar_color),
    ));
    spans.push(Span::styled(
        "░".repeat(empty),
        Style::default().add_modifier(Modifier::DIM),
    ));
    spans.push(Span::styled(
        format!(" {}%", state.percent),
        Style::default(),
    ));

    // Separator
    spans.push(Span::raw(" │ "));

    // Indicators
    if state.think_enabled {
        spans.push(Span::raw("🧠"));
    }
    if state.tools_enabled {
        spans.push(Span::raw("🔧"));
    }
    // Style rendering indicator — always visible (option B) so users
    // discover the /toggle-style command. 🎨 means styled (diagrams,
    // syntax highlighting, box-drawing tables), 📄 means source/raw.
    if state.style_enabled {
        spans.push(Span::raw("🎨"));
    } else {
        spans.push(Span::raw("📄"));
    }

    // Embedding progress — shown after indicators with separator
    if let Some(progress) = state.embedding_progress {
        let phase_emoji = match progress.phase {
            EmbeddingPhase::Content => "📄",
            EmbeddingPhase::Facts => "💡",
            EmbeddingPhase::FactDedup => "🔍",
        };
        let ahead = progress.embeddings_current > progress.entities_current;
        spans.push(Span::raw(" │ "));
        spans.push(Span::styled(
            "⚙ ",
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{}/{}", progress.entities_current, progress.entities_total),
            Style::default()
                .fg(styles::YELLOW)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {} · ", phase_emoji)));
        spans.push(Span::styled(
            format!(
                "{}/{}{}",
                progress.embeddings_current,
                progress.embeddings_total,
                if ahead { "↗" } else { "" }
            ),
            Style::default()
                .fg(styles::CYAN)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Right-aligned overlay (retry warning, etc.)
    if let Some(ref overlay) = state.overlay {
        let overlay_text = format!("  {}  ", overlay);
        let used_width: usize = spans.iter().map(|s| s.content.width()).sum();
        let overlay_width = overlay_text.width();
        let padding = area
            .width
            .saturating_sub((used_width + overlay_width) as u16) as usize;
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding)));
        }
        spans.push(Span::styled(
            overlay_text,
            Style::default()
                .fg(styles::RED)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Build the line
    let line = Line::from(spans);

    // Render separator above
    let separator = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().add_modifier(Modifier::DIM),
    ));

    let paragraph = Paragraph::new(vec![separator, line]);
    f.render_widget(paragraph, area);
}
