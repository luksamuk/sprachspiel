//! Chat area component — scrollable message display
//!
//! Renders all chat messages, welcome info, thinking blocks,
//! tool calls, and command outputs in a scrollable area.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::super::markdown::{MarkdownTheme, render_markdown};
use super::super::styles;

/// A single message in the chat area
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Role label (e.g., "You", "Assistant", "System", "Error", "Thinking")
    pub role: String,
    /// Message content (plain text during streaming, markdown after completion)
    pub content: String,
    /// Whether this is a thinking block (rendered dimmed)
    pub is_thinking: bool,
    /// Whether this content should be rendered as markdown
    pub is_markdown: bool,
}

impl ChatMessage {
    /// Create a user message
    pub fn user(content: String) -> Self {
        Self {
            role: "You".to_string(),
            content,
            is_thinking: false,
            is_markdown: false,
        }
    }

    /// Create an assistant message (plain text for streaming)
    #[allow(dead_code)] // PR3: Will be used for streaming assistant messages in TUI
    pub fn assistant_streaming(content: String) -> Self {
        Self {
            role: "Assistant".to_string(),
            content,
            is_thinking: false,
            is_markdown: false,
        }
    }

    /// Create an assistant message (markdown after completion)
    pub fn assistant_markdown(content: String) -> Self {
        Self {
            role: "Assistant".to_string(),
            content,
            is_thinking: false,
            is_markdown: true,
        }
    }

    /// Create a thinking block
    pub fn thinking(content: String) -> Self {
        Self {
            role: "Thinking".to_string(),
            content,
            is_thinking: true,
            is_markdown: false,
        }
    }

    /// Create a system message
    pub fn system(content: String) -> Self {
        Self {
            role: "System".to_string(),
            content,
            is_thinking: false,
            is_markdown: false,
        }
    }

    /// Create an error message
    pub fn error(content: String) -> Self {
        Self {
            role: "Error".to_string(),
            content,
            is_thinking: false,
            is_markdown: false,
        }
    }
}

/// Render the chat area component
///
/// Displays all messages in a scrollable list. Uses ratatui paragraph
/// rendering. Markdown messages are rendered with `tui-markdown` when
/// complete, and plain text during streaming.
pub fn render(
    f: &mut Frame,
    area: Rect,
    messages: &[ChatMessage],
    scroll_offset: u16,
    theme: MarkdownTheme,
) {
    let mut lines: Vec<ratatui::text::Line> = Vec::new();

    for msg in messages {
        // Label line
        let label_style = if msg.is_thinking {
            styles::thinking_label_style()
        } else {
            match msg.role.as_str() {
                "You" => styles::user_label_style(),
                "Assistant" => styles::assistant_label_style(),
                "Error" => styles::error_style(),
                _ => styles::system_style(),
            }
        };

        lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(format!("[{}] ", msg.role), label_style),
        ]));

        // Content lines
        if msg.is_markdown {
            // Render markdown content
            let rendered = render_markdown(&msg.content, theme);
            for line in rendered.lines {
                lines.push(line);
            }
        } else if msg.is_thinking {
            // Render thinking content as dimmed lines
            let content_style = styles::thinking_content_style();
            for line in msg.content.lines() {
                lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("  {}", line),
                    content_style,
                )));
            }
        } else {
            // Render plain text
            for line in msg.content.lines() {
                lines.push(ratatui::text::Line::raw(line.to_string()));
            }
        }

        // Blank line between messages
        lines.push(ratatui::text::Line::raw(String::new()));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0));

    f.render_widget(paragraph, area);
}
