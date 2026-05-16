//! Completion menu component — floating overlay for tab completions
//!
//! Renders a floating menu above the status bar when multiple tab
//! completions are available. The menu is NOT part of the layout
//! (no layout shift) — it's rendered as an overlay above the input
//! area using `ratatui::widgets::Clear`.
//!
//! # Design
//!
//! - Left-aligned, 80% width when terminal ≥ 60 cols, 100% when < 60
//! - Shows completion items with the common prefix highlighted
//! - Appears above the status bar (between chat area and input)
//! - Disappears when the user types or presses Enter/Esc

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::super::styles;

/// Minimum terminal width for 80% menu width (vs 100% for narrow terminals)
const WIDE_TERMINAL_THRESHOLD: u16 = 60;

/// Width percentage for wide terminals
const WIDE_WIDTH_PERCENT: u16 = 80;

/// Maximum visible completion items (scroll if more)
const MAX_VISIBLE_ITEMS: u16 = 8;

/// State for the completion menu overlay
#[derive(Debug, Clone)]
pub struct CompletionMenuState {
    /// Available completion items
    items: Vec<String>,
    /// Currently selected index (0-based)
    selected: usize,
    /// Scroll offset for long lists
    scroll_offset: usize,
    /// The common prefix that all items share
    common_prefix: String,
    /// Whether the menu is visible
    visible: bool,
}

impl CompletionMenuState {
    /// Create a new empty completion menu state
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            common_prefix: String::new(),
            visible: false,
        }
    }

    /// Show the menu with the given items and common prefix
    pub fn show(&mut self, items: Vec<String>, common_prefix: String) {
        if items.is_empty() {
            self.visible = false;
            return;
        }
        self.items = items;
        self.common_prefix = common_prefix;
        self.selected = 0;
        self.scroll_offset = 0;
        self.visible = true;
    }

    /// Hide the menu
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Whether the menu is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selected).map(|s| s.as_str())
    }

    /// Move selection up (wrap around)
    pub fn select_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.items.len() - 1;
        }
        self.adjust_scroll();
    }

    /// Move selection down (wrap around)
    pub fn select_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected < self.items.len() - 1 {
            self.selected += 1;
        } else {
            self.selected = 0;
        }
        self.adjust_scroll();
    }

    /// Confirm the current selection and return it
    pub fn confirm(&mut self) -> Option<String> {
        let item = self.selected_item().map(|s| s.to_string());
        self.hide();
        item
    }

    /// Adjust scroll offset to keep the selected item visible
    fn adjust_scroll(&mut self) {
        let visible = MAX_VISIBLE_ITEMS as usize;
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible {
            self.scroll_offset = self.selected - visible + 1;
        }
    }

    /// Get the number of items
    #[allow(dead_code)] // Public API for external state queries
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no items
    #[allow(dead_code)] // Public API for external state queries
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for CompletionMenuState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the completion menu as a floating overlay
///
/// The menu appears above the input area, between the chat area and the
/// status bar. It's rendered using `Clear` to wipe the underlying content,
/// then drawing the menu widget on top.
///
/// # Arguments
///
/// * `f` - The ratatui frame
/// * `status_bar_area` - The area of the status bar (menu floats above it)
/// * `state` - The completion menu state
pub fn render_overlay(f: &mut Frame, status_bar_area: Rect, state: &CompletionMenuState) {
    if !state.is_visible() || state.items.is_empty() {
        return;
    }

    // Calculate menu dimensions
    let terminal_width = f.area().width;
    let menu_width = if terminal_width >= WIDE_TERMINAL_THRESHOLD {
        terminal_width * WIDE_WIDTH_PERCENT / 100
    } else {
        terminal_width
    };

    // Menu height: number of items, capped at MAX_VISIBLE_ITEMS, plus 1 for border
    let visible_count = state.items.len().min(MAX_VISIBLE_ITEMS as usize) as u16;
    let menu_height = visible_count + 2; // +2 for top/bottom border

    // Position: above the status bar, left-aligned
    let menu_y = status_bar_area.y.saturating_sub(menu_height);
    let menu_x = status_bar_area.x;

    let menu_area = Rect {
        x: menu_x,
        y: menu_y,
        width: menu_width.min(terminal_width),
        height: menu_height,
    };

    // Clear the area where the menu will be drawn
    f.render_widget(Clear, menu_area);

    // Build the menu lines
    let prefix_len = state.common_prefix.len();
    let selected_style = Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED);
    let prefix_style = styles::bold_cyan();
    let normal_style = Style::default();

    let mut lines: Vec<Line> = Vec::new();
    let visible_items: Vec<&String> = state
        .items
        .iter()
        .skip(state.scroll_offset)
        .take(MAX_VISIBLE_ITEMS as usize)
        .collect();

    for (i, item) in visible_items.iter().enumerate() {
        let actual_index = state.scroll_offset + i;
        let is_selected = actual_index == state.selected;
        let item_style = if is_selected {
            selected_style
        } else {
            normal_style
        };

        // Highlight the common prefix in cyan, rest in normal/selected style
        let (prefix_part, rest_part) = if prefix_len > 0 && item.len() > prefix_len {
            (&item[..prefix_len], &item[prefix_len..])
        } else if prefix_len > 0 {
            (item.as_str(), "")
        } else {
            ("", item.as_str())
        };

        // Style for the prefix: highlighted when selected, cyan otherwise
        let prefix_display_style = if is_selected {
            selected_style
        } else {
            prefix_style
        };

        lines.push(Line::from(vec![
            Span::styled(prefix_part.to_string(), prefix_display_style),
            Span::styled(rest_part.to_string(), item_style),
        ]));
    }

    // Render with a block border
    let block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().add_modifier(Modifier::DIM))
        .title(" Completions ");
    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, menu_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_menu_new() {
        let state = CompletionMenuState::new();
        assert!(!state.is_visible());
        assert!(state.is_empty());
    }

    #[test]
    fn test_completion_menu_show_hide() {
        let mut state = CompletionMenuState::new();
        state.show(
            vec!["help".to_string(), "history".to_string()],
            "h".to_string(),
        );
        assert!(state.is_visible());
        assert_eq!(state.len(), 2);
        assert_eq!(state.selected_item(), Some("help"));

        state.hide();
        assert!(!state.is_visible());
    }

    #[test]
    fn test_completion_menu_navigation() {
        let mut state = CompletionMenuState::new();
        state.show(
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            String::new(),
        );

        assert_eq!(state.selected, 0);
        state.select_down();
        assert_eq!(state.selected, 1);
        state.select_down();
        assert_eq!(state.selected, 2);
        // Wrap around
        state.select_down();
        assert_eq!(state.selected, 0);

        state.select_up();
        assert_eq!(state.selected, 2);
        state.select_up();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn test_completion_menu_confirm() {
        let mut state = CompletionMenuState::new();
        state.show(vec!["hello".to_string()], "h".to_string());
        let result = state.confirm();
        assert_eq!(result, Some("hello".to_string()));
        assert!(!state.is_visible());
    }

    #[test]
    fn test_completion_menu_show_empty() {
        let mut state = CompletionMenuState::new();
        state.show(Vec::new(), String::new());
        assert!(!state.is_visible());
    }

    #[test]
    fn test_completion_menu_scroll_adjustment() {
        let mut state = CompletionMenuState::new();
        let items: Vec<String> = (0..20).map(|i| format!("item_{}", i)).collect();
        state.show(items, String::new());

        // Select item 15 — should adjust scroll
        state.selected = 15;
        state.adjust_scroll();
        // Scroll should jump to keep item 15 visible
        assert!(state.scroll_offset <= 15);
    }
}
