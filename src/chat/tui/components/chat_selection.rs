//! Chat text selection component — mouse-based text selection in the chat area
//!
//! Tracks selection state (start/end positions in visual lines) and provides:
//! - Mouse click/drag handling to create and extend selections
//! - Selection text extraction (for clipboard copy)
//! - Highlight rendering (background color on selected ranges)
//!
//! # Coordinate System
//!
//! Selection uses **visual line coordinates**: (visual_line_index, char_offset).
//! Visual lines are the wrapped lines as rendered on screen (after word-wrap).
//! The `chat_area::render()` function produces the same visual lines, so
//! selection coordinates map directly to the rendered content.
//!
//! # Mouse Mapping
//!
//! Mouse events give (column, row) in terminal coordinates. To map to
//! visual line coordinates:
//! - `visual_line = mouse_row - chat_area_top + scroll_offset`
//! - `char_offset = mouse_col - chat_area_left` (simplified, no wide-char accounting)

/// Style for selected text (reverse video: white text on blue background)
pub fn selection_style() -> ratatui::style::Style {
    ratatui::style::Style::default()
        .fg(ratatui::style::Color::White)
        .bg(ratatui::style::Color::Blue)
}

/// State for chat text selection.
///
/// Tracks the start and end positions of the selection in visual-line
/// coordinates. The start is always ≤ end (normalized on every update).
/// The anchor is the fixed point (where the mouse click started); the
/// end moves with drag.
#[derive(Debug, Clone, Default)]
pub struct ChatSelection {
    /// Whether a selection is active (text is highlighted)
    active: bool,
    /// The anchor point (where the mouse button was pressed)
    anchor: (usize, usize), // (visual_line, char_offset)
    /// The cursor point (where the mouse currently is / was released)
    cursor: (usize, usize), // (visual_line, char_offset)
    /// Selection mode: true = currently dragging (mouse button held)
    dragging: bool,
}

impl ChatSelection {
    /// Create a new empty (inactive) selection
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a selection at the given position (mouse down)
    pub fn begin(&mut self, visual_line: usize, char_offset: usize) {
        self.active = true;
        self.anchor = (visual_line, char_offset);
        self.cursor = (visual_line, char_offset);
        self.dragging = true;
    }

    /// Extend the selection to a new position (mouse drag)
    pub fn extend(&mut self, visual_line: usize, char_offset: usize) {
        if !self.dragging {
            return;
        }
        self.cursor = (visual_line, char_offset);
    }

    /// Finish the selection (mouse up) — stops dragging but keeps selection
    pub fn finish(&mut self, visual_line: usize, char_offset: usize) {
        if self.dragging {
            self.cursor = (visual_line, char_offset);
            self.dragging = false;
            // If anchor == cursor (click without drag), it's a zero-width selection — deactivate
            if self.anchor == self.cursor {
                self.active = false;
            }
        }
    }

    /// Clear the selection (deactivate)
    pub fn clear(&mut self) {
        self.active = false;
        self.dragging = false;
        self.anchor = (0, 0);
        self.cursor = (0, 0);
    }

    /// Whether the selection is active (has a highlighted range)
    pub fn is_active(&self) -> bool {
        self.active && self.selection_start() != self.selection_end()
    }

    /// Whether the selection is currently being dragged
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Get the start of the selection (normalized: start ≤ end)
    pub fn selection_start(&self) -> (usize, usize) {
        let (al, ac) = self.anchor;
        let (cl, cc) = self.cursor;
        if al < cl || (al == cl && ac <= cc) {
            (al, ac)
        } else {
            (cl, cc)
        }
    }

    /// Get the end of the selection (normalized: start ≤ end)
    pub fn selection_end(&self) -> (usize, usize) {
        let (al, ac) = self.anchor;
        let (cl, cc) = self.cursor;
        if al < cl || (al == cl && ac <= cc) {
            (cl, cc)
        } else {
            (al, ac)
        }
    }

    /// Extract the selected text from a list of visual line strings.
    ///
    /// Given the flat list of visual lines (after word-wrap), returns
    /// the text covered by the selection.
    pub fn extract_text(&self, visual_lines: &[String]) -> String {
        if !self.is_active() || visual_lines.is_empty() {
            return String::new();
        }

        let (start_line, start_col) = self.selection_start();
        let (end_line, end_col) = self.selection_end();

        // Clamp to available lines
        let start_line = start_line.min(visual_lines.len().saturating_sub(1));
        let end_line = end_line.min(visual_lines.len().saturating_sub(1));

        if start_line == end_line {
            // Single-line selection
            let line = &visual_lines[start_line];
            let start = start_col.min(line.len());
            let end = end_col.min(line.len());
            line[start..end].to_string()
        } else {
            // Multi-line selection
            let mut result = String::new();

            // First line: from start_col to end
            let first = &visual_lines[start_line];
            let s = start_col.min(first.len());
            result.push_str(&first[s..]);
            result.push('\n');

            // Middle lines: entire content
            for line in visual_lines.iter().take(end_line).skip(start_line + 1) {
                result.push_str(line);
                result.push('\n');
            }

            // Last line: from beginning to end_col
            let last = &visual_lines[end_line];
            let e = end_col.min(last.len());
            result.push_str(&last[..e]);

            result
        }
    }
}

/// Map a mouse event position to visual-line coordinates within the chat area.
///
/// Converts terminal (col, row) to (visual_line, char_offset) given the
/// chat area's position, scroll state, and total wrapped line count.
///
/// Returns `None` if the click is outside the chat area.
pub fn mouse_to_visual_pos(
    mouse_col: u16,
    mouse_row: u16,
    chat_area: ratatui::layout::Rect,
    scroll_from_top: u16,
) -> Option<(usize, usize)> {
    // Check if click is within the chat area
    if mouse_col < chat_area.x
        || mouse_col >= chat_area.x + chat_area.width
        || mouse_row < chat_area.y
        || mouse_row >= chat_area.y + chat_area.height
    {
        return None;
    }

    // Convert to local coordinates within the chat area
    let local_row = mouse_row - chat_area.y;
    let local_col = mouse_col - chat_area.x;

    // Add scroll offset to get the visual line index
    let visual_line = local_row as usize + scroll_from_top as usize;
    let char_offset = local_col as usize;

    Some((visual_line, char_offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_start_extend_finish() {
        let mut sel = ChatSelection::new();
        assert!(!sel.is_active());

        // Start selection
        sel.begin(2, 5);
        assert!(sel.is_dragging());
        // Zero-width (anchor == cursor) is NOT active
        assert!(!sel.is_active());

        // Extend changes cursor
        sel.extend(4, 10);
        assert!(sel.is_active());
        assert!(sel.is_dragging());
        assert_eq!(sel.selection_start(), (2, 5));
        assert_eq!(sel.selection_end(), (4, 10));

        // Finish stops dragging but keeps selection
        sel.finish(4, 12);
        assert!(!sel.is_dragging());
        assert!(sel.is_active());
        assert_eq!(sel.selection_start(), (2, 5));
        assert_eq!(sel.selection_end(), (4, 12));
    }

    #[test]
    fn test_selection_backward_drag() {
        // Drag from (5, 10) backward to (3, 2)
        let mut sel = ChatSelection::new();
        sel.begin(5, 10);
        sel.extend(3, 2);

        // start/end should be normalized
        assert_eq!(sel.selection_start(), (3, 2));
        assert_eq!(sel.selection_end(), (5, 10));
    }

    #[test]
    fn test_selection_clear() {
        let mut sel = ChatSelection::new();
        sel.begin(1, 0);
        sel.extend(5, 10);
        assert!(sel.is_active());

        sel.clear();
        assert!(!sel.is_active());
        assert!(!sel.is_dragging());
    }

    #[test]
    fn test_selection_click_without_drag() {
        let mut sel = ChatSelection::new();
        sel.begin(3, 5);
        // Finish immediately at the same position
        sel.finish(3, 5);
        // Zero-width selection deactivates
        assert!(!sel.is_active());
    }

    #[test]
    fn test_extract_text_single_line() {
        let sel = {
            let mut s = ChatSelection::new();
            s.begin(1, 5);
            s.extend(1, 9);
            s.finish(1, 9);
            s
        };
        let lines = vec![
            "Hello world".to_string(),
            "This is a test".to_string(),
            "Goodbye".to_string(),
        ];
        assert_eq!(sel.extract_text(&lines), "is a");
    }

    #[test]
    fn test_extract_text_multi_line() {
        let sel = {
            let mut s = ChatSelection::new();
            s.begin(0, 6);
            s.extend(2, 4);
            s.finish(2, 4);
            s
        };
        let lines = vec![
            "Hello world".to_string(),
            "This is a test".to_string(),
            "Goodbye".to_string(),
        ];
        assert_eq!(sel.extract_text(&lines), "world\nThis is a test\nGood");
    }

    #[test]
    fn test_extract_text_out_of_bounds() {
        let sel = {
            let mut s = ChatSelection::new();
            s.begin(0, 0);
            s.extend(10, 50);
            s.finish(10, 50);
            s
        };
        let lines = vec!["Hello".to_string(), "World".to_string()];
        // Should not panic — clamps to available lines
        let text = sel.extract_text(&lines);
        assert_eq!(text, "Hello\nWorld");
    }

    #[test]
    fn test_mouse_to_visual_pos_inside() {
        let area = ratatui::layout::Rect::new(0, 5, 80, 20);
        let result = mouse_to_visual_pos(10, 8, area, 3);
        assert_eq!(result, Some((6, 10))); // row 8 - area.y 5 + scroll 3 = 6, col 10
    }

    #[test]
    fn test_mouse_to_visual_pos_outside_left() {
        let area = ratatui::layout::Rect::new(5, 5, 80, 20);
        let result = mouse_to_visual_pos(3, 8, area, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_mouse_to_visual_pos_outside_bottom() {
        let area = ratatui::layout::Rect::new(0, 5, 80, 20);
        let result = mouse_to_visual_pos(10, 30, area, 0);
        assert_eq!(result, None);
    }
}
