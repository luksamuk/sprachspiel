//! Terminal display formatting for embedding diagnostics
//!
//! Renders the results of spectral analysis as a Markdown report,
//! then formats it using the standalone Markdown renderer for
//! rich terminal output (ANSI bold, box-drawing tables) or plain
//! text (pipe-delimited tables, no ANSI codes).

use super::embeddings::{EmbeddingDiagnostics, Regime};

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

/// Format embedding diagnostics as a Markdown string with explanatory notes.
///
/// Each section includes a blockquote (`>`) explaining the metric in plain language,
/// so even users unfamiliar with embedding geometry can interpret the results.
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
        md.push_str("> ⚠ No embedding vectors found in the database.\n");
        md.push_str("> Run the chat to generate embeddings first, or check the --db path.\n");
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
        "Concentrated — most dimensions are redundant. The embeddings compress information into very few directions."
    } else if d_eff_pct >= 50.0 {
        "Diffuse — information is spread across many dimensions. The embedding space is well-utilized."
    } else {
        "Balanced — the embeddings use dimensions efficiently without over-concentrating."
    };
    md.push_str(&format!("> {}\n\n", d_eff_interp));
    md.push_str("> The effective dimensionality measures how many of the nominal dimensions actually carry signal. A d_eff of ");
    md.push_str(&format!(
        "{:.1} out of {} means the embeddings concentrate information in roughly {:.0} directions — ",
        diag.d_eff, diag.nominal_dimensions, diag.d_eff
    ));
    if diag.d_eff < diag.nominal_dimensions as f64 * 0.2 {
        md.push_str("the remaining dimensions carry mostly noise.\n\n");
    } else {
        md.push_str("the remaining dimensions carry varying amounts of signal.\n\n");
    }

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

    md.push_str("> Cosine distance ranges from 0 (identical vectors) to 2 (opposite vectors). ");
    if diag.mean_cosine_distance < 0.3 {
        md.push_str("A mean d̄ below 0.3 indicates vectors are tightly clustered — similar items may be hard to distinguish.\n\n");
    } else if diag.mean_cosine_distance > 1.0 {
        md.push_str("A mean d̄ above 1.0 indicates highly dispersed vectors — search may return poorly differentiated results.\n\n");
    } else {
        md.push_str("A mean d̄ around 0.3–1.0 indicates well-spread vectors with good discriminative power.\n\n");
    }

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

    md.push_str("> **TIGHT** means vector search discriminates well at that threshold — similar items cluster close together. **SPREAD** means results are not well differentiated.\n\n");

    if tight_count == total_thresholds {
        md.push_str("✅ Embedding geometry is TIGHT at all thresholds. Vector search discriminates well.\n\n");
    } else if tight_count == 0 {
        md.push_str("⚠ Embedding geometry is SPREAD at all thresholds. Vector search provides minimal discrimination.\n");
        md.push_str("Consider using a different embedding model or increasing corpus size.\n\n");
    } else {
        md.push_str(&format!(
            "⚡ Mixed geometry — TIGHT at {}/{} thresholds, SPREAD at others. Search discriminates at higher similarity thresholds.\n\n",
            tight_count, total_thresholds
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
        md.push_str(&format!(
            "> **Concentrated:** 90% of variance is in {} principal components (of {} dimensions). Most dimensions are redundant.\n\n",
            ve.pc_90, diag.nominal_dimensions
        ));
    } else if ve.pc_90 > diag.nominal_dimensions / 4 {
        md.push_str(&format!(
            "> **Diffuse:** 90% of variance requires {} principal components (of {} dimensions). Information spreads across dimensions.\n\n",
            ve.pc_90, diag.nominal_dimensions
        ));
    } else {
        md.push_str(&format!(
            "> **Balanced:** 90% of variance is in {} principal components (of {} dimensions).\n\n",
            ve.pc_90, diag.nominal_dimensions
        ));
    }

    // Small corpus warning
    if diag.vector_count < 100 {
        md.push_str(&format!(
            "⚠ *Only {} vectors analyzed. d_eff estimates may be unreliable (max d_eff = {}). Collect ≥100 vectors for stable estimates.*\n\n",
            diag.vector_count,
            diag.vector_count.saturating_sub(1)
        ));
    }

    // Large corpus note
    if diag.vector_count > 5000 {
        md.push_str(&format!(
            "ℹ *Large corpus ({} vectors). Pairwise analysis is O(n²), results took several seconds to compute.*\n\n",
            diag.vector_count
        ));
    }

    md
}
