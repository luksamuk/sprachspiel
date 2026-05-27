//! Terminal display formatting for embedding diagnostics
//!
//! Renders the results of spectral analysis as a Markdown report,
//! then formats it using the standalone Markdown renderer for
//! rich terminal output (ANSI bold, box-drawing tables) or plain
//! text (pipe-delimited tables, no ANSI codes).

use super::embeddings::{EmbeddingDiagnostics, Regime};
use crate::settings::{DEFAULT_KEYWORD_WEIGHT, DEFAULT_SEMANTIC_WEIGHT};

/// Maximum content width for blockquote lines before wrapping.
///
/// The markdown renderer adds `│ ` (2 chars + space) as prefix for
/// blockquotes, and the terminal is typically 80 columns wide.
/// 76 chars of content + 4 chars of prefix = 80 columns.
const BLOCKQUOTE_CONTENT_WIDTH: usize = 76;

/// Display embedding diagnostics to stdout, using Markdown rendering.
///
/// When `plain` is true, outputs without ANSI codes (pipe-delimited tables).
/// When `plain` is false, uses rich formatting (ANSI bold headings, box-drawing tables).
pub fn display_diagnostics(diag: &EmbeddingDiagnostics, plain: bool) {
    let markdown = format_diagnostics_markdown(diag);
    if plain {
        crate::markdown::print_markdown_plain(&markdown);
    } else {
        crate::markdown::print_markdown(&markdown);
    }
}

/// Wrap a paragraph of text to a maximum line width.
///
/// Splits on word boundaries (whitespace). Long words that exceed
/// `max_width` are placed on their own line without splitting.
fn wrap_text(text: &str, max_width: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            result.push_str(&current_line);
            result.push('\n');
            current_line.clear();
            current_line.push_str(word);
        }
    }
    if !current_line.is_empty() {
        result.push_str(&current_line);
    }

    result
}

/// Format a paragraph as Markdown blockquote lines, wrapped to `BLOCKQUOTE_CONTENT_WIDTH`.
///
/// Each line of the wrapped text gets a `> ` prefix. A blank `>` line is
/// added before and after to create visual separation in the rendered output.
fn blockquote(text: &str) -> String {
    let wrapped = wrap_text(text, BLOCKQUOTE_CONTENT_WIDTH);
    let mut result = String::with_capacity(wrapped.len() + wrapped.len() / 20);
    result.push_str("> ");
    for line in wrapped.lines() {
        result.push_str(line);
        result.push_str("\n> ");
    }
    // Remove trailing "> " and add just newline
    if result.ends_with("> ") {
        result.truncate(result.len() - 2);
    }
    result.push('\n');
    result
}

/// Format embedding diagnostics as a Markdown string with explanatory notes.
///
/// Each section includes a blockquote (`>`) explaining the metric in plain language,
/// so even users unfamiliar with embedding geometry can interpret the results.
/// Long blockquote paragraphs are word-wrapped to 76 columns for readability.
pub fn format_diagnostics_markdown(diag: &EmbeddingDiagnostics) -> String {
    let mut md = String::new();

    // Header
    md.push_str("# Embedding Diagnostics Report\n\n");
    md.push_str(&format!("**Model:** {}\n", diag.model_name));
    md.push_str(&format!("**Dimensions:** {}\n\n", diag.nominal_dimensions));

    // Vector counts
    md.push_str("## Vector counts\n\n");
    md.push_str("| Source | Count |\n");
    md.push_str("|--------|-------|\n");
    let mut total = 0usize;
    for (source, count) in &diag.source_counts {
        md.push_str(&format!("| {} | {} |\n", source, count));
        total += count;
    }
    md.push_str(&format!("| **total** | **{}** |\n\n", total));

    if diag.vector_count == 0 {
        md.push_str(&blockquote(
            "⚠ No embedding vectors found in the database. \
             Run the chat to generate embeddings first, or check the --db path.",
        ));
        return md;
    }

    // Spectral analysis
    md.push_str("## Spectral analysis\n\n");
    md.push_str(&format!(
        "**d_eff (participation ratio):** {:.1} / {} ({:.1}%)\n\n",
        diag.d_eff, diag.nominal_dimensions, diag.d_eff_percent
    ));

    // d_eff interpretation
    let d_eff_pct = diag.d_eff_percent;
    let d_eff_interp = if d_eff_pct <= 20.0 {
        "Concentrated — most dimensions are redundant. \
         The embeddings compress information into very few directions."
    } else if d_eff_pct >= 50.0 {
        "Diffuse — information is spread across many dimensions. \
         The embedding space is well-utilized."
    } else {
        "Balanced — the embeddings use dimensions efficiently \
         without over-concentrating."
    };
    md.push_str(&blockquote(d_eff_interp));

    let d_eff_detail = format!(
        "The effective dimensionality measures how many of the {} \
         nominal dimensions actually carry signal. A d_eff of {:.1} \
         out of {} means the embeddings concentrate information in \
         roughly {:.0} directions",
        diag.nominal_dimensions, diag.d_eff, diag.nominal_dimensions, diag.d_eff,
    );
    let d_eff_tail = if diag.d_eff < diag.nominal_dimensions as f64 * 0.2 {
        " — the remaining dimensions carry mostly noise."
    } else {
        " — the remaining dimensions carry varying amounts of signal."
    };
    md.push_str(&blockquote(&format!("{}{}", d_eff_detail, d_eff_tail)));

    // Pairwise cosine distance
    md.push_str("## Pairwise cosine distance\n\n");
    md.push_str("| Statistic | Value |\n");
    md.push_str("|-----------|-------|\n");
    md.push_str(&format!(
        "| Mean (d̄) | {:.6} |\n",
        diag.mean_cosine_distance
    ));
    md.push_str(&format!("| Min | {:.6} |\n", diag.min_cosine_distance));
    md.push_str(&format!("| Max | {:.6} |\n\n", diag.max_cosine_distance));

    let cd_interp = if diag.mean_cosine_distance < 0.3 {
        "Cosine distance ranges from 0 (identical vectors) to 2 (opposite \
         vectors). A mean d̄ below 0.3 indicates vectors are tightly \
         clustered — similar items may be hard to distinguish."
    } else if diag.mean_cosine_distance > 1.0 {
        "Cosine distance ranges from 0 (identical vectors) to 2 (opposite \
         vectors). A mean d̄ above 1.0 indicates highly dispersed vectors \
         — search may return poorly differentiated results."
    } else {
        "Cosine distance ranges from 0 (identical vectors) to 2 (opposite \
         vectors). A mean d̄ around 0.3–1.0 indicates well-spread vectors \
         with good discriminative power."
    };
    md.push_str(&blockquote(cd_interp));

    // Regime classification
    md.push_str("## Regime classification\n\n");
    md.push_str("| Threshold (θ) | θ' | d̄ | Regime |\n");
    md.push_str("|---------------|------|--------|--------|\n");
    for r in &diag.regimes {
        md.push_str(&format!(
            "| {:.2} | {:.2} | {:.6} | {} |\n",
            r.theta, r.theta_prime, diag.mean_cosine_distance, r.regime
        ));
    }
    md.push('\n');

    // Regime interpretation
    let tight_count = diag
        .regimes
        .iter()
        .filter(|r| r.regime == Regime::Tight)
        .count();
    let total_thresholds = diag.regimes.len();

    md.push_str(&blockquote(
        "**TIGHT** means vector search discriminates well at that \
         threshold — similar items cluster close together. **SPREAD** \
         means results are not well differentiated.",
    ));

    if tight_count == total_thresholds {
        md.push_str(
            "✅ Embedding geometry is TIGHT at all thresholds. \
             Vector search discriminates well.\n\n",
        );
    } else if tight_count == 0 {
        md.push_str(
            "⚠ Embedding geometry is SPREAD at all thresholds. \
             Vector search provides minimal discrimination.\n",
        );
        md.push_str(
            "Consider using a different embedding model or \
             increasing corpus size.\n\n",
        );
    } else {
        md.push_str(&format!(
            "⚡ Mixed geometry — TIGHT at {}/{} thresholds, \
             SPREAD at others. Search discriminates at higher \
             similarity thresholds.\n\n",
            tight_count, total_thresholds,
        ));
    }

    // Variance explained
    md.push_str("## Variance explained\n\n");
    md.push_str("| Cumulative % | Principal Component |\n");
    md.push_str("|--------------|---------------------|\n");
    md.push_str(&format!(
        "| 50% | PC #{} |\n",
        diag.variance_explained.pc_50
    ));
    md.push_str(&format!(
        "| 90% | PC #{} |\n",
        diag.variance_explained.pc_90
    ));
    md.push_str(&format!(
        "| 95% | PC #{} |\n",
        diag.variance_explained.pc_95
    ));
    md.push_str(&format!(
        "| 99% | PC #{} |\n\n",
        diag.variance_explained.pc_99
    ));

    let ve = &diag.variance_explained;
    let five_pct_dims = (diag.nominal_dimensions as f64 * 0.05) as usize;
    if ve.pc_90 <= five_pct_dims.max(5) {
        md.push_str(&blockquote(&format!(
            "**Concentrated:** 90% of variance is in {} principal \
             components (of {} dimensions). Most dimensions are \
             redundant.",
            ve.pc_90, diag.nominal_dimensions,
        )));
    } else if ve.pc_90 > diag.nominal_dimensions / 4 {
        md.push_str(&blockquote(&format!(
            "**Diffuse:** 90% of variance requires {} principal \
             components (of {} dimensions). Information spreads \
             across dimensions.",
            ve.pc_90, diag.nominal_dimensions,
        )));
    } else {
        md.push_str(&blockquote(&format!(
            "**Balanced:** 90% of variance is in {} principal \
             components (of {} dimensions).",
            ve.pc_90, diag.nominal_dimensions,
        )));
    }

    // Threshold recommendations
    let rec = &diag.threshold_recommendation;
    md.push_str("## Recommended configuration\n\n");
    md.push_str(&format!(
        "**[facts].semantic_threshold:** {:.2}\n\n",
        rec.semantic_threshold
    ));
    md.push_str(&blockquote(&rec.rationale));
    md.push('\n');

    if rec.adjust_weights {
        md.push_str(&format!(
            "**[retrieval].keyword_weight:** {:.1}\n\n",
            rec.suggested_keyword_weight
        ));
        md.push_str(&format!(
            "**[retrieval].semantic_weight:** {:.1}\n\n",
            rec.suggested_semantic_weight
        ));
        md.push_str(&blockquote(&rec.weight_rationale));
        md.push('\n');
    } else {
        md.push_str(&blockquote(&format!(
            "Default weights (keyword={:.1}, semantic={:.1}) are appropriate \
             for the current embedding geometry.",
            DEFAULT_KEYWORD_WEIGHT, DEFAULT_SEMANTIC_WEIGHT
        )));
        md.push('\n');
    }

    md.push_str(&blockquote(
        "To apply these recommendations, update your config.toml \
         or run `sprach config edit`. These are informational \
         suggestions based on observed embedding geometry.",
    ));
    md.push('\n');

    // Small corpus warning
    if diag.vector_count < 100 {
        md.push_str(&format!(
            "⚠ *Only {} vectors analyzed. d_eff estimates may be \
             unreliable (max d_eff = {}). Collect ≥100 vectors for \
             stable estimates.*\n\n",
            diag.vector_count,
            diag.vector_count.saturating_sub(1),
        ));
    }

    // Large corpus note
    if diag.vector_count > 5000 {
        md.push_str(&format!(
            "ℹ *Large corpus ({} vectors). Pairwise analysis is \
             O(n²), results took several seconds to compute.*\n\n",
            diag.vector_count,
        ));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text_short_line() {
        let text = "Hello world";
        let wrapped = wrap_text(text, 80);
        assert_eq!(wrapped, "Hello world");
    }

    #[test]
    fn test_wrap_text_long_line() {
        let text = "This is a long line that should be wrapped at word boundaries to fit within the specified width limit";
        let wrapped = wrap_text(text, 40);
        for line in wrapped.lines() {
            assert!(
                line.len() <= 40,
                "Line too long ({}): '{}'",
                line.len(),
                line
            );
        }
        // Should be wrapped into multiple lines
        assert!(wrapped.lines().count() > 1);
    }

    #[test]
    fn test_wrap_text_at_boundary() {
        // 10 chars per word × 3 words + 2 spaces = 32 chars total
        let text = "1234567890 1234567890 1234567890";
        assert_eq!(text.len(), 32);

        // Exactly fits on one line at width 32
        let wrapped32 = wrap_text(text, 32);
        assert_eq!(wrapped32.lines().count(), 1);
        assert_eq!(wrapped32, text);

        // Just overflows at width 31 → two lines
        let wrapped31 = wrap_text(text, 31);
        assert_eq!(wrapped31.lines().count(), 2);

        // Tight fit: width 20 → three lines (one word per line)
        let wrapped20 = wrap_text(text, 20);
        assert_eq!(wrapped20.lines().count(), 3);
    }

    #[test]
    fn test_wrap_text_single_long_word() {
        let text = "supercalifragilisticexpialidocious";
        let wrapped = wrap_text(text, 20);
        // Long word should appear on its own line
        assert_eq!(wrapped, "supercalifragilisticexpialidocious");
    }

    #[test]
    fn test_blockquote_format() {
        let text = "This is a short explanation.";
        let result = blockquote(text);
        assert!(result.starts_with("> "));
        assert!(result.contains("This is a short explanation."));
    }

    #[test]
    fn test_blockquote_wraps_long_text() {
        let text = "This is a very long explanation that should be wrapped to fit within the blockquote content width limit of seventy-six characters per line.";
        let result = blockquote(text);
        for line in result.lines() {
            // Each line starts with "> " (2 chars) and content should be ≤76 + 2 = 78,
            // but we check content after "> " prefix
            if let Some(content) = line.strip_prefix("> ") {
                assert!(
                    content.len() <= BLOCKQUOTE_CONTENT_WIDTH,
                    "Blockquote line too long ({}): '{}'",
                    content.len(),
                    content
                );
            }
        }
    }
}
