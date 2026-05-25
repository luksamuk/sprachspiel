//! Terminal display formatting for embedding diagnostics
//!
//! Renders the results of spectral analysis as a human-readable report
//! to stdout, including d_eff, d̄, regime classification, and variance
//! distribution.

#![expect(clippy::print_stdout)] // CLI diagnostics output
#![expect(clippy::print_literal)] // Diagnostic output with variable text

use super::embeddings::EmbeddingDiagnostics;

/// Display embedding diagnostics to stdout
pub fn display_diagnostics(diag: &EmbeddingDiagnostics) {
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║          Embedding Diagnostics Report                    ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Model and corpus overview
    println!("  Model:              {}", diag.model_name);
    println!("  Nominal dimensions: {}", diag.nominal_dimensions);
    println!();

    // Vector counts by source
    println!("  ── Vector counts ──");
    let mut total = 0usize;
    for (source, count) in &diag.source_counts {
        println!("    {:<16} {}", format!("{}:", source), count);
        total += count;
    }
    println!("    {:<16} {}", "total:", total);

    if diag.vector_count == 0 {
        println!();
        println!("  ⚠ No embedding vectors found in the database.");
        println!("  Run the chat to generate embeddings first, or check the --db path.");
        return;
    }
    println!();

    // Spectral analysis results
    println!("  ── Spectral analysis ──");
    println!(
        "    d_eff (participation ratio): {:.1} / {} ({:.1}%)",
        diag.d_eff, diag.nominal_dimensions, diag.d_eff_percent
    );
    println!(
        "    Effective dimensionality:    {:.1}% of nominal",
        diag.d_eff_percent
    );
    println!();

    // Pairwise cosine distance statistics
    println!("  ── Pairwise cosine distance ──");
    println!("    Mean (d̄):  {:.6}", diag.mean_cosine_distance);
    println!("    Min:       {:.6}", diag.min_cosine_distance);
    println!("    Max:       {:.6}", diag.max_cosine_distance);
    println!();

    // Regime classification
    println!("  ── Regime classification ──");
    println!(
        "    {:<12} {:<10} {:<12} {}",
        "Threshold", "θ'", "d̄", "Regime"
    );
    for r in &diag.regimes {
        println!(
            "    θ={:.2}       {:.2}     {:.6}     {}",
            r.theta, r.theta_prime, diag.mean_cosine_distance, r.regime
        );
    }
    println!();

    // Interpretation guide
    let dominant_regime = diag
        .regimes
        .iter()
        .filter(|r| r.regime == super::embeddings::Regime::Tight)
        .count();

    if dominant_regime >= 3 {
        println!("  ✅ Embedding geometry is TIGHT at most thresholds.");
        println!("     Vector search discriminates well — similar items cluster tightly.");
    } else if dominant_regime == 0 {
        println!("  ⚠ Embedding geometry is SPREAD at all thresholds.");
        println!("     Vector search provides minimal discrimination.");
        println!("     Consider: using a different embedding model, or increasing corpus size.");
    } else {
        println!("  ⚡ Mixed geometry — TIGHT at some thresholds, SPREAD at others.");
        println!("     Vector search discriminates at higher similarity thresholds.");
    }
    println!();

    // Variance explained
    println!("  ── Variance explained ──");
    println!("    PC reaching 50%:  #{}", diag.variance_explained.pc_50);
    println!("    PC reaching 90%:  #{}", diag.variance_explained.pc_90);
    println!("    PC reaching 95%:  #{}", diag.variance_explained.pc_95);
    println!("    PC reaching 99%:  #{}", diag.variance_explained.pc_99);

    // Interpretation
    let ve = &diag.variance_explained;
    if ve.pc_90 <= 5 {
        println!(
            "     → Concentrated: 90% variance in {} PCs (of {} dim)",
            ve.pc_90, diag.nominal_dimensions
        );
    } else if ve.pc_90 > diag.nominal_dimensions / 4 {
        println!(
            "     → Diffuse: 90% variance requires {} PCs (of {} dim)",
            ve.pc_90, diag.nominal_dimensions
        );
    } else {
        println!(
            "     → Balanced: 90% variance in {} PCs (of {} dim)",
            ve.pc_90, diag.nominal_dimensions
        );
    }
    println!();

    // Small corpus warning
    if diag.vector_count < 100 {
        println!(
            "  ⚠ Note: Only {} vectors analyzed. d_eff estimates may be unreliable",
            diag.vector_count
        );
        println!(
            "    (max d_eff = n-1 = {}). Collect ≥100 vectors for stable estimates.",
            diag.vector_count.saturating_sub(1)
        );
        println!();
    }

    // Large corpus scalability note
    if diag.vector_count > 5000 {
        println!(
            "  ℹ Large corpus ({} vectors). Pairwise analysis is O(n²),",
            diag.vector_count
        );
        println!("    results may take several seconds.");
        println!();
    }
}
