//! Task-steering prefix templates for asymmetric retrieval.
//!
//! Embedding models like embeddinggemma-2 are trained to rank query and
//! document texts in different input formats. The configuration carries two
//! templates — `[indexing].query_prefix` for the query side and
//! `[indexing].doc_prefix` for the document side — with explicit
//! `{content}` and `{title}` placeholders. This module validates those
//! templates against the placeholder contract at config-load time and
//! renders them opaquely at embed time.
//!
//! # Placeholder contract
//!
//! - `{content}` must appear exactly once in a non-empty template.
//! - `{title}` is valid only in the doc template (queries have no title).
//! - `None`/empty/whitespace titles render as `none`.
//! - Any other `{...}` token is a configuration error.
//!
//! # Opaque substitution
//!
//! Values are substituted in a single left-to-right pass over the
//! template, and the substituted values are never re-scanned: content
//! containing a literal `{title}` reaches the server untouched.

/// Value rendered for a missing, empty, or whitespace-only title.
pub const NO_TITLE_VALUE: &str = "none";

/// An invalid prefix template, with a message naming the config key.
#[derive(Debug, Clone)]
pub struct TemplateError(pub String);

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid embedding prefix template: {}", self.0)
    }
}

impl std::error::Error for TemplateError {}

/// Rendered steering templates for the retrieval's two sides.
///
/// An empty side renders the text unchanged (zero behavior change by
/// default).
#[derive(Debug, Clone, Default)]
pub struct PrefixTemplates {
    /// Template for query-side texts. Only `{content}` is valid here.
    pub query: String,
    /// Template for document-side texts. `{content}` required exactly
    /// once; `{title}` optional.
    pub doc: String,
}

impl PrefixTemplates {
    /// Validate both templates against the placeholder contract, naming
    /// the offending config key in the error message.
    pub fn validate(&self) -> Result<(), TemplateError> {
        Self::validate_one(&self.query, false, "[indexing].query_prefix")?;
        Self::validate_one(&self.doc, true, "[indexing].doc_prefix")
    }

    fn validate_one(
        template: &str,
        title_allowed: bool,
        key: &str,
    ) -> Result<(), TemplateError> {
        if template.is_empty() {
            return Ok(());
        }
        let placeholders = scan_placeholders(template)
            .map_err(|e| TemplateError(format!("{key}: {e}")))?;
        let content_count = placeholders.iter().filter(|p| **p == "content").count();
        if content_count != 1 {
            return Err(TemplateError(format!(
                "{key} must contain the {{content}} placeholder exactly once, \
                 found {content_count}"
            )));
        }
        let has_title = placeholders.contains(&"title");
        if has_title && !title_allowed {
            return Err(TemplateError(format!(
                "{key} may not use the {{{{title}}}} placeholder — queries \
                 have no title"
            )));
        }
        Ok(())
    }

    /// Render the query-side template around `content`.
    pub fn render_query(&self, content: &str) -> String {
        render(&self.query, None, content)
    }

    /// Render the doc-side template around `content` with an optional
    /// title. A missing, empty, or whitespace-only title renders as
    /// `none`.
    pub fn render_doc(&self, title: Option<&str>, content: &str) -> String {
        render(&self.doc, title, content)
    }

    /// Worst-case length of the rendered prefixes, in characters, with
    /// substitutions at their empty/`none` forms. Used to size the
    /// chunker's token margin so the prefix never displaces content
    /// silently.
    pub fn max_rendered_len(&self) -> usize {
        self.render_query("")
            .len()
            .max(self.render_doc(None, "").len())
    }
}

/// Scan a template for `{...}` placeholders, erroring on unknown names
/// and on an unclosed brace.
fn scan_placeholders(template: &str) -> Result<Vec<&str>, String> {
    let mut found = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open + 1..];
        match after_open.find('}') {
            Some(close) => {
                let name = &after_open[..close];
                match name {
                    "content" | "title" => found.push(name),
                    other => {
                        return Err(format!(
                            "unknown placeholder {{{{{other}}}}} — only \
                             {{content}} and {{title}} are valid"
                        ))
                    }
                }
                rest = &after_open[close + 1..];
            }
            None => {
                return Err("unclosed '{' with no matching '}'".to_string());
            }
        }
    }
    Ok(found)
}

/// Render `template` by substituting placeholders in a single left-to-right
/// pass. `title` and `content` are opaque values: a title or content
/// containing literal `{...}` placeholders is never re-substituted.
fn render(template: &str, title: Option<&str>, content: &str) -> String {
    if template.is_empty() {
        return content.to_string();
    }
    let title_value = match title {
        Some(t) if !t.trim().is_empty() => t,
        _ => NO_TITLE_VALUE,
    };

    let mut out = String::with_capacity(template.len() + content.len() + title_value.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let close = match rest[open..].find('}') {
            Some(c) => open + c,
            // After validation this is unreachable; pass the rest through.
            None => break,
        };
        out.push_str(&rest[..open]);
        match &rest[open + 1..close] {
            "content" => out.push_str(content),
            "title" => out.push_str(title_value),
            // Unreachable for validated templates; the placeholder token
            // is dropped rather than crashing at embed time.
            _ => {}
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_templates_render_input_unchanged() {
        let t = PrefixTemplates::default();
        assert!(t.validate().is_ok());
        assert_eq!(t.render_query("hello"), "hello");
        assert_eq!(t.render_doc(Some("Title"), "hello"), "hello");
        assert_eq!(t.render_doc(None, "hello"), "hello");
        assert_eq!(t.max_rendered_len(), 0);
    }

    #[test]
    fn test_query_template_requires_content_exactly_once() {
        let t = PrefixTemplates {
            query: "task: search result | query: {content}".to_string(),
            doc: String::new(),
        };
        assert!(t.validate().is_ok());

        let missing = PrefixTemplates {
            query: "task: search result".to_string(),
            doc: String::new(),
        };
        let err = missing.validate().unwrap_err();
        assert!(err.0.contains("query_prefix") && err.0.contains("exactly"));

        let duplicated = PrefixTemplates {
            query: "{content} vs {content}".to_string(),
            doc: String::new(),
        };
        assert!(duplicated.validate().is_err());
    }

    #[test]
    fn test_doc_template_requires_content_exactly_once() {
        let t = PrefixTemplates {
            query: String::new(),
            doc: "title: {title} | text: {content}".to_string(),
        };
        assert!(t.validate().is_ok());

        let t = PrefixTemplates {
            query: String::new(),
            doc: "title: none | text: missing content".to_string(),
        };
        assert!(t.validate().is_err());
    }

    #[test]
    fn test_title_only_valid_in_doc_template() {
        let t = PrefixTemplates {
            query: "q: {content} title: {title}".to_string(),
            doc: String::new(),
        };
        let err = t.validate().unwrap_err();
        assert!(err.0.contains("query_prefix"));
    }

    #[test]
    fn test_unknown_placeholder_is_config_error() {
        let t = PrefixTemplates {
            query: "task: search result | query: {query}".to_string(),
            doc: String::new(),
        };
        let err = t.validate().unwrap_err();
        assert!(err.0.contains("{query}"));
    }

    #[test]
    fn test_unclosed_brace_is_config_error() {
        let t = PrefixTemplates {
            query: "task: search result | query: {content".to_string(),
            doc: String::new(),
        };
        assert!(t.validate().is_err());
    }

    #[test]
    fn test_render_query_exact() {
        let t = PrefixTemplates {
            query: "task: search result | query: {content}".to_string(),
            doc: String::new(),
        };
        assert_eq!(
            t.render_query("rust async traits"),
            "task: search result | query: rust async traits"
        );
    }

    #[test]
    fn test_render_doc_titled_exact() {
        let t = PrefixTemplates {
            query: String::new(),
            doc: "title: {title} | text: {content}".to_string(),
        };
        assert_eq!(
            t.render_doc(Some("Design Doc"), "the body"),
            "title: Design Doc | text: the body"
        );
    }

    #[test]
    fn test_render_doc_untitled_renders_none() {
        let t = PrefixTemplates {
            query: String::new(),
            doc: "title: {title} | text: {content}".to_string(),
        };
        assert_eq!(
            t.render_doc(None, "the body"),
            "title: none | text: the body"
        );
        assert_eq!(
            t.render_doc(Some("   "), "the body"),
            "title: none | text: the body"
        );
        assert_eq!(
            t.render_doc(Some(""), "the body"),
            "title: none | text: the body"
        );
    }

    #[test]
    fn test_render_values_are_opaque_no_chained_substitution() {
        let t = PrefixTemplates {
            query: "q: {content}".to_string(),
            doc: "title: {title} | text: {content}".to_string(),
        };
        // Content containing a placeholder must reach the server literally.
        assert_eq!(
            t.render_query("user wrote {title} in braces"),
            "q: user wrote {title} in braces"
        );
        // Title containing {content} must not be re-substituted with itself.
        assert_eq!(
            t.render_doc(Some("{content}"), "body"),
            "title: {content} | text: body"
        );
    }

    #[test]
    fn test_max_rendered_len_sizes_the_worst_case() {
        let t = PrefixTemplates {
            query: "task: search result | query: ".to_string(),
            doc: "title: none | text: ".to_string(),
        };
        // Substitutions collapsed to their empty/none forms.
        assert_eq!(t.max_rendered_len(), "task: search result | query: ".len());
        let doc_only = PrefixTemplates {
            query: String::new(),
            doc: "a{title}b{content}c".to_string(),
        };
        // {title} → none (4), {content} → "" (0).
        assert_eq!(doc_only.max_rendered_len(), 1 + 4 + 1 + 1);
    }
}
