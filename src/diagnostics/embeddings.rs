//! Spectral analysis for embedding diagnostics
//!
//! Computes effective dimensionality (d_eff), mean cosine distance (d̄),
//! regime classification, and variance distribution from stored embedding vectors.
//!
//! # Algorithms
//!
//! All algorithms use pure-Rust implementations with no external dependencies:
//!
//! - **d_eff (Participation Ratio):** `(Σλᵢ)² / Σλᵢ²` where λᵢ are eigenvalues
//!   of the covariance matrix. Uses power iteration with deflation for the top
//!   k eigenvalues, and the matrix trace for the total sum.
//!
//! - **d̄ (Mean Cosine Distance):** `1 - mean(Gᵢⱼ)` for i≠j, where G = X·Xᵀ
//!   is the Gram matrix of normalized vectors.
//!
//! - **Regime Classification:** SPREAD if `d̄ ≥ (1 - θ)`, TIGHT otherwise.
//!   Based on "The Geometry of Forgetting" (Barman et al., 2026).
//!
//! - **Variance Explained:** Cumulative eigenvalue sum / total eigenvalue sum,
//!   reporting which principal component reaches 50%, 90%, 95%, 99%.

use std::fmt;

use clap::ValueEnum;

use crate::settings::{
    DEFAULT_KEYWORD_WEIGHT, DEFAULT_SEMANTIC_THRESHOLD, DEFAULT_SEMANTIC_WEIGHT,
};

/// Source of embedding vectors for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EmbeddingSource {
    /// Content item embeddings (messages, notes, documents)
    Content,
    /// Chunk embeddings (long content split into segments)
    Chunks,
    /// Fact embeddings (factual memory system)
    Facts,
}

impl fmt::Display for EmbeddingSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbeddingSource::Content => write!(f, "content"),
            EmbeddingSource::Chunks => write!(f, "chunks"),
            EmbeddingSource::Facts => write!(f, "facts"),
        }
    }
}

/// Regime classification at a specific threshold
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    /// Vector search has minimal discriminative power at this threshold
    Spread,
    /// Vector search provides meaningful discrimination at this threshold
    Tight,
}

impl fmt::Display for Regime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Regime::Spread => write!(f, "SPREAD"),
            Regime::Tight => write!(f, "TIGHT"),
        }
    }
}

/// Regime result at a specific threshold
#[derive(Debug, Clone)]
pub struct RegimeAtThreshold {
    /// Similarity threshold θ
    pub theta: f64,
    /// Complement of θ: θ' = 1 - θ
    pub theta_prime: f64,
    /// Whether the regime is SPREAD or TIGHT
    pub regime: Regime,
}

/// Recommended configuration values based on embedding geometry analysis.
///
/// Based on the observed d_eff and d̄, this struct recommends:
/// - A `semantic_threshold` for fact deduplication that avoids false positives
///   in SPREAD regimes while maintaining good recall in TIGHT regimes.
/// - Whether `keyword_weight` / `semantic_weight` should be adjusted from defaults
///   based on the observed regime.
#[derive(Debug, Clone)]
pub struct ThresholdRecommendation {
    /// Recommended `[facts].semantic_threshold` value
    pub semantic_threshold: f64,
    /// Rationale for the recommendation
    pub rationale: String,
    /// Whether default weights are appropriate, or if the user should adjust them
    pub adjust_weights: bool,
    /// Suggested `keyword_weight` (only meaningful if `adjust_weights` is true)
    pub suggested_keyword_weight: f64,
    /// Suggested `semantic_weight` (only meaningful if `adjust_weights` is true)
    pub suggested_semantic_weight: f64,
    /// Weight adjustment rationale (only meaningful if `adjust_weights` is true)
    pub weight_rationale: String,
}

/// Variance explained at specific cumulative percentages
#[derive(Debug, Clone)]
pub struct VarianceExplained {
    /// Principal component number where 50% of variance is explained
    pub pc_50: usize,
    /// Principal component number where 90% of variance is explained
    pub pc_90: usize,
    /// Principal component number where 95% of variance is explained
    pub pc_95: usize,
    /// Principal component number where 99% of variance is explained
    pub pc_99: usize,
}

/// Complete diagnostics results for embedding analysis
#[derive(Debug, Clone)]
pub struct EmbeddingDiagnostics {
    /// Number of embedding vectors analyzed
    pub vector_count: usize,
    /// Breakdown by source (content, chunks, facts)
    pub source_counts: Vec<(EmbeddingSource, usize)>,
    /// Nominal dimensions of stored vectors (e.g., 256)
    pub nominal_dimensions: usize,
    /// Effective dimensionality (participation ratio)
    pub d_eff: f64,
    /// d_eff as percentage of nominal dimensions
    pub d_eff_percent: f64,
    /// Mean cosine distance between all pairs
    pub mean_cosine_distance: f64,
    /// Minimum cosine distance between any pair
    pub min_cosine_distance: f64,
    /// Maximum cosine distance between any pair
    pub max_cosine_distance: f64,
    /// Regime classification at each threshold
    pub regimes: Vec<RegimeAtThreshold>,
    /// Variance explained at key percentiles
    pub variance_explained: VarianceExplained,
    /// Embedding model name
    pub model_name: String,
    /// Recommended threshold values based on observed geometry
    pub threshold_recommendation: ThresholdRecommendation,
}

/// Progress callback type for spectral analysis.
///
/// Called with `(phase_name, progress_fraction)` where `progress_fraction`
/// is 0.0–1.0. Used to update a progress bar during long-running computations.
///
/// For a corpus of 30K vectors, the spectral analysis phases and their
/// approximate cost are:
/// - Phase 1 "Centering data" — ~1% (O(n·d))
/// - Phase 2 "Computing covariance matrix" — ~50% (O(n²·d))
/// - Phase 3 "Computing eigenvalues" — ~5% (O(k·n²))
/// - Phase 4 "Computing pairwise distances" — ~44% (O(n²·d))
pub type ProgressFn = dyn Fn(&str, f64);

/// Analyze embedding vectors and compute diagnostics (without progress reporting).
///
/// Convenience wrapper that calls [`analyze_embeddings_with_progress`] with a
/// no-op callback. Use this in tests and when progress reporting is not needed.
#[cfg(test)]
pub fn analyze_embeddings(
    vectors: &[Vec<f64>],
    nominal_dimensions: usize,
    model_name: &str,
    source_counts: Vec<(EmbeddingSource, usize)>,
) -> EmbeddingDiagnostics {
    analyze_embeddings_with_progress(
        vectors,
        nominal_dimensions,
        model_name,
        source_counts,
        &|_, _| {},
    )
}

/// Analyze embedding vectors and compute diagnostics with progress reporting.
///
/// Takes a matrix of embedding vectors (each row is a vector) and computes
/// d_eff, d̄, regime classification, and variance distribution.
///
/// # Arguments
/// * `vectors` - Matrix of embedding vectors (N × D, f64)
/// * `nominal_dimensions` - Original embedding dimensionality (e.g., 256)
/// * `model_name` - Name of the embedding model
/// * `source_counts` - Breakdown by source
/// * `progress` - Callback called with (phase_name, progress_fraction 0.0–1.0)
///
/// # Caveats
/// * With n < 100 vectors, d_eff estimates are unreliable (max d_eff = n-1)
/// * With n < 2 vectors, d̄ and min/max cannot be computed
pub fn analyze_embeddings_with_progress(
    vectors: &[Vec<f64>],
    nominal_dimensions: usize,
    model_name: &str,
    source_counts: Vec<(EmbeddingSource, usize)>,
    progress: &ProgressFn,
) -> EmbeddingDiagnostics {
    let n = vectors.len();

    if n == 0 {
        return EmbeddingDiagnostics {
            vector_count: 0,
            source_counts,
            nominal_dimensions,
            d_eff: 0.0,
            d_eff_percent: 0.0,
            mean_cosine_distance: 0.0,
            min_cosine_distance: 0.0,
            max_cosine_distance: 0.0,
            regimes: vec![],
            variance_explained: VarianceExplained {
                pc_50: 0,
                pc_90: 0,
                pc_95: 0,
                pc_99: 0,
            },
            model_name: model_name.to_string(),
            threshold_recommendation: ThresholdRecommendation {
                semantic_threshold: DEFAULT_SEMANTIC_THRESHOLD as f64,
                rationale: "No vectors available for analysis. Using default threshold."
                    .to_string(),
                adjust_weights: false,
                suggested_keyword_weight: DEFAULT_KEYWORD_WEIGHT as f64,
                suggested_semantic_weight: DEFAULT_SEMANTIC_WEIGHT as f64,
                weight_rationale: String::new(),
            },
        };
    }

    let d = vectors[0].len();

    // Phase 1: Centering (1% of total work)
    progress("Centering data", 0.0);
    let mean = compute_mean(vectors, d);
    let centered = center_vectors(vectors, &mean);

    // Phase 2: Covariance/Gram matrix (51% of total work)
    // The matrix computation is the most expensive step: O(n²·d)
    progress("Computing covariance matrix", 0.01);
    let total_variance: f64 = centered
        .iter()
        .map(|v| v.iter().map(|x| x * x).sum::<f64>())
        .sum::<f64>()
        / n as f64;

    // Phase 2b: Eigenvalue computation via power iteration (5% of total work)
    progress("Computing eigenvalues", 0.52);
    let (eigenvalues, d_eff) = if total_variance == 0.0 {
        (Vec::new(), 0.0)
    } else {
        compute_d_eff_from_centered(&centered, n, d, total_variance)
    };

    // Phase 3: Pairwise cosine distance (44% of total work)
    progress("Computing pairwise distances", 0.57);
    let (mean_cd, min_cd, max_cd) =
        compute_cosine_distance_stats_with_progress(vectors, n, d, progress, 0.57, 1.0);

    // Phase 4: Regime classification and variance (trivial)
    progress("Finalizing analysis", 0.99);
    let regimes = compute_regimes(mean_cd);
    let variance_explained = compute_variance_explained(&eigenvalues, d);
    let threshold_recommendation = recommend_threshold(d_eff, mean_cd, &regimes);

    EmbeddingDiagnostics {
        vector_count: n,
        source_counts,
        nominal_dimensions,
        d_eff,
        d_eff_percent: if nominal_dimensions > 0 {
            d_eff / nominal_dimensions as f64 * 100.0
        } else {
            0.0
        },
        mean_cosine_distance: mean_cd,
        min_cosine_distance: min_cd,
        max_cosine_distance: max_cd,
        regimes,
        variance_explained,
        model_name: model_name.to_string(),
        threshold_recommendation,
    }
}

/// Convert f32 vectors to f64 for numerical stability in SVD
pub fn vectors_f32_to_f64(vectors: &[Vec<f32>]) -> Vec<Vec<f64>> {
    vectors
        .iter()
        .map(|v| v.iter().map(|x| *x as f64).collect())
        .collect()
}

/// Compute d_eff (participation ratio) and eigenvalues via power iteration with deflation
///
/// d_eff = (Σλᵢ)² / Σλᵢ²
///
/// For n < d (fewer vectors than dimensions), uses the Gram matrix approach:
/// eigenvalues of the N×N Gram matrix (divided by n) equal the non-zero
/// eigenvalues of the D×D covariance matrix.
///
/// For n ≥ d, uses the covariance matrix directly.
#[cfg(test)]
fn compute_d_eff(vectors: &[Vec<f64>], n: usize, d: usize) -> (Vec<f64>, f64) {
    if n < 2 || d < 1 {
        return (Vec::new(), 0.0);
    }

    let mean = compute_mean(vectors, d);
    let centered = center_vectors(vectors, &mean);

    let total_variance: f64 = centered
        .iter()
        .map(|v| v.iter().map(|x| x * x).sum::<f64>())
        .sum::<f64>()
        / n as f64;

    if total_variance == 0.0 {
        return (Vec::new(), 0.0);
    }

    compute_d_eff_from_centered(&centered, n, d, total_variance)
}

/// Compute d_eff from already-centered vectors.
///
/// Separated from `compute_d_eff` so that `analyze_embeddings_with_progress`
/// can reuse the centered vectors without recomputing them.
fn compute_d_eff_from_centered(
    centered: &[Vec<f64>],
    n: usize,
    d: usize,
    total_variance: f64,
) -> (Vec<f64>, f64) {
    // Number of eigenvalues to compute via power iteration
    let k = (d.min(n - 1)).min(50);

    // Compute top-k eigenvalues via power iteration
    let eigenvalues = power_iteration_eigenvalues(centered, k, 100, 1e-10);

    // For d_eff, we need (Σλᵢ)² / Σλᵢ²
    let sum_lambda = total_variance;

    // Σλᵢ² = sum of squared eigenvalues (from computed ones + residual)
    let sum_computed: f64 = eigenvalues.iter().sum::<f64>();
    let sum_lambda_sq_computed: f64 = eigenvalues.iter().map(|x| x.powi(2)).sum::<f64>();

    // Residual eigenvalues (those not computed by power iteration)
    // After centering, max rank = min(d, n-1), not d. Using d would
    // distribute residual variance across dimensions that must be zero,
    // inflating d_eff for small corpora.
    let max_rank = d.min(n - 1);
    let residual_variance = (total_variance - sum_computed).max(0.0);
    let residual_count = max_rank.saturating_sub(k);
    let residual_each = if residual_count > 0 {
        residual_variance / residual_count as f64
    } else {
        0.0
    };
    let sum_lambda_sq_residual = residual_count as f64 * residual_each.powi(2);

    let sum_lambda_sq = sum_lambda_sq_computed + sum_lambda_sq_residual;

    let d_eff = if sum_lambda_sq > 0.0 {
        sum_lambda.powi(2) / sum_lambda_sq
    } else {
        0.0
    };

    (eigenvalues, d_eff)
}

/// Compute mean vector from a set of vectors
fn compute_mean(vectors: &[Vec<f64>], d: usize) -> Vec<f64> {
    let n = vectors.len() as f64;
    let mut mean = vec![0.0; d];
    for v in vectors {
        for (j, slot) in mean.iter_mut().enumerate().take(d) {
            *slot += v[j];
        }
    }
    for slot in mean.iter_mut().take(d) {
        *slot /= n;
    }
    mean
}

/// Center vectors by subtracting the mean
fn center_vectors(vectors: &[Vec<f64>], mean: &[f64]) -> Vec<Vec<f64>> {
    vectors
        .iter()
        .map(|v| v.iter().zip(mean.iter()).map(|(x, m)| x - m).collect())
        .collect()
}

/// Power iteration with deflation to compute top-k eigenvalues
///
/// Returns eigenvalues in descending order.
/// Internally dispatches between covariance (D×D) and Gram (N×N) approaches.
fn power_iteration_eigenvalues(
    centered: &[Vec<f64>],
    k: usize,
    max_iter: usize,
    tolerance: f64,
) -> Vec<f64> {
    let n = centered.len();
    if n == 0 || k == 0 {
        return Vec::new();
    }
    let d = centered[0].len();

    let mut eigenvalues = Vec::with_capacity(k);

    if d <= n {
        // Direct covariance matrix approach (D×D matrix)
        let cov = compute_covariance_matrix(centered);
        let mut deflated = cov.clone();

        for i in 0..k {
            let (eigenvalue, eigenvector) =
                power_iteration_with_eigenvector(&deflated, d, max_iter, tolerance, i as u64);
            let eigenvalue = eigenvalue.max(0.0);
            eigenvalues.push(eigenvalue);

            // Deflate: remove this component from the matrix
            for i in 0..d {
                for j in 0..d {
                    deflated[i][j] -= eigenvalue * eigenvector[i] * eigenvector[j];
                }
            }
        }
    } else {
        // Gram matrix approach (N×N matrix, more efficient when n < d)
        let gram = compute_gram_matrix(centered);
        let n_f = n as f64;
        let mut deflated = gram.clone();

        for i in 0..k.min(n) {
            let (eigenvalue, eigenvector) =
                power_iteration_with_eigenvector(&deflated, n, max_iter, tolerance, i as u64);
            let eigenvalue_cov = eigenvalue.max(0.0) / n_f;
            eigenvalues.push(eigenvalue_cov);

            // Deflate using the Gram eigenvalue (not scaled)
            let eigenvalue_gram = eigenvalue.max(0.0);
            for i in 0..n {
                for j in 0..n {
                    deflated[i][j] -= eigenvalue_gram * eigenvector[i] * eigenvector[j];
                }
            }
        }
    }

    eigenvalues
}

/// Run power iteration and return both the eigenvalue and eigenvector
///
/// Returns (eigenvalue, normalized_eigenvector) using Rayleigh quotient
/// for better eigenvalue estimation.
///
/// Uses a pseudo-random starting vector (seeded by `seed`) to avoid
/// pathological cases where a standard basis vector is orthogonal to
/// an eigenspace. For matrices with degenerate eigenvalues, different
/// seeds explore different directions in the eigenspace.
fn power_iteration_with_eigenvector(
    matrix: &[Vec<f64>],
    dim: usize,
    max_iter: usize,
    tolerance: f64,
    seed: u64,
) -> (f64, Vec<f64>) {
    // Pseudo-random starting vector using a simple LCG.
    // This avoids the pathological case where a standard basis vector
    // is orthogonal to an eigenspace, which causes power iteration to
    // miss eigenvalues or produce garbage after deflation.
    let mut rng_state = seed;
    let mut vec = vec![0.0; dim];
    for x in vec.iter_mut() {
        // LCG: x_{n+1} = 6364136223846793005 * x_n + 1442695040888963407 (Knuth)
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Map to [-1, 1] via simple bit manipulation
        *x = (rng_state as i64 as f64) / (i64::MAX as f64);
    }

    // Normalize the starting vector
    let norm: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm < tolerance {
        // Fallback to standard basis if random vector is degenerate
        vec = vec![0.0; dim];
        vec[0] = 1.0;
    } else {
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }

    for _ in 0..max_iter {
        // Matrix-vector product
        let mut new_vec = vec![0.0; dim];
        for i in 0..dim {
            for j in 0..dim {
                new_vec[i] += matrix[i][j] * vec[j];
            }
        }

        // Normalize
        let norm: f64 = new_vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm < tolerance {
            break;
        }
        for x in new_vec.iter_mut() {
            *x /= norm;
        }

        // Check convergence
        let diff: f64 = new_vec
            .iter()
            .zip(vec.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f64, |acc, x| acc + x);

        vec = new_vec;

        if diff < tolerance {
            break;
        }
    }

    // Compute eigenvalue via Rayleigh quotient for better accuracy
    let eigenvalue = compute_rayleigh_quotient(matrix, &vec);
    (eigenvalue, vec)
}

/// Compute D×D covariance matrix from centered vectors
fn compute_covariance_matrix(centered: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = centered.len() as f64;
    let d = centered[0].len();

    let mut cov = vec![vec![0.0; d]; d];

    for v in centered {
        for i in 0..d {
            for j in i..d {
                let prod = v[i] * v[j];
                cov[i][j] += prod / n;
                if i != j {
                    cov[j][i] += prod / n;
                }
            }
        }
    }

    cov
}

/// Compute N×N Gram matrix from vectors (for n < d case)
fn compute_gram_matrix(vectors: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = vectors.len();
    let mut gram = vec![vec![0.0; n]; n];

    for i in 0..n {
        for j in i..n {
            let dot: f64 = vectors[i]
                .iter()
                .zip(vectors[j].iter())
                .map(|(a, b)| a * b)
                .sum();
            gram[i][j] = dot;
            gram[j][i] = dot;
        }
    }

    gram
}

/// Compute Rayleigh quotient (eigenvalue estimate) for a vector
fn compute_rayleigh_quotient(matrix: &[Vec<f64>], v: &[f64]) -> f64 {
    let dim = v.len();
    let mut av = vec![0.0; dim];
    for i in 0..dim {
        for j in 0..dim {
            av[i] += matrix[i][j] * v[j];
        }
    }

    // Rayleigh quotient = (v^T * A * v) / (v^T * v)
    let numerator: f64 = v.iter().zip(av.iter()).map(|(a, b)| a * b).sum();
    let denominator: f64 = v.iter().map(|x| x * x).sum();

    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

/// Compute cosine distance statistics between all pairs of vectors
///
/// For L2-normalized vectors, cosine similarity = dot product.
/// Cosine distance = 1 - cosine_similarity.
///
/// Convenience wrapper without progress reporting. Used in tests.
#[cfg(test)]
fn compute_cosine_distance_stats(vectors: &[Vec<f64>], n: usize, _d: usize) -> (f64, f64, f64) {
    compute_cosine_distance_stats_with_progress(vectors, n, _d, &|_, _| {}, 0.0, 1.0)
}

/// Compute cosine distance statistics with progress reporting.
///
/// The `progress` callback is called at intervals during the O(n²) computation.
/// `start_frac` and `end_frac` define the fraction of total work this phase represents.
fn compute_cosine_distance_stats_with_progress(
    vectors: &[Vec<f64>],
    n: usize,
    _d: usize,
    progress: &ProgressFn,
    start_frac: f64,
    end_frac: f64,
) -> (f64, f64, f64) {
    if n < 2 {
        return (0.0, 0.0, 0.0);
    }

    let mut sum = 0.0;
    let mut count = 0usize;
    let mut min_cd = f64::MAX;
    let mut max_cd = f64::NEG_INFINITY;

    // Report progress every ~1% of the outer loop
    let report_interval = (n / 100).max(1);

    for i in 0..n {
        for j in (i + 1)..n {
            let cos_sim = dot_product(&vectors[i], &vectors[j]);
            let cos_dist = 1.0 - cos_sim;
            sum += cos_dist;
            count += 1;
            min_cd = min_cd.min(cos_dist);
            max_cd = max_cd.max(cos_dist);
        }

        // Report progress periodically
        if i % report_interval == 0 && i > 0 {
            let frac = start_frac + (end_frac - start_frac) * (i as f64 / n as f64);
            progress("Computing pairwise distances", frac);
        }
    }

    // Final progress report for this phase
    progress("Computing pairwise distances", end_frac);

    let mean_cd = if count > 0 { sum / count as f64 } else { 0.0 };
    (mean_cd, min_cd, max_cd)
}

/// Compute dot product of two vectors
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Compute regime classification at standard thresholds
///
/// At each threshold θ:
/// - θ' = 1 - θ (complement)
/// - If d̄ ≥ θ' → SPREAD (vectors too spread for threshold to discriminate)
/// - If d̄ < θ' → TIGHT (threshold provides meaningful discrimination)
fn compute_regimes(mean_cosine_distance: f64) -> Vec<RegimeAtThreshold> {
    let thresholds = [0.70_f64, 0.75, 0.80, 0.85];

    thresholds
        .iter()
        .map(|&theta| {
            let theta_prime = 1.0 - theta;
            let regime = if mean_cosine_distance >= theta_prime {
                Regime::Spread
            } else {
                Regime::Tight
            };
            RegimeAtThreshold {
                theta,
                theta_prime,
                regime,
            }
        })
        .collect()
}

/// Recommend a semantic threshold and weight configuration based on observed geometry.
///
/// The recommendation follows these rules:
///
/// - **TIGHT at θ=0.70:** Default threshold 0.70 is appropriate. Vectors
///   discriminate well at this level. No weight adjustment needed.
///
/// - **SPREAD at θ=0.70 but TIGHT at θ=0.80:** Recommend raising threshold
///   to 0.80. Vectors don't discriminate well enough at 0.70 (many false
///   positives in dedup), but work well at 0.80. Consider increasing
///   keyword_weight since semantic search is less reliable at lower thresholds.
///
/// - **SPREAD at all thresholds (0.70–0.85):** The embedding space has
///   minimal discriminative power. Recommend 0.85 threshold and shifting
///   strongly toward keyword-matching (keyword_weight=0.7, semantic_weight=0.3).
///
/// - **Edge case — very low d̄ (< 0.15):** Vectors are nearly identical.
///   This usually means the corpus is too small or too homogeneous. Threshold
///   doesn't matter much; suggest 0.70 with a warning.
pub fn recommend_threshold(
    _d_eff: f64,
    mean_cosine_distance: f64,
    regimes: &[RegimeAtThreshold],
) -> ThresholdRecommendation {
    let tight_at_070 = regimes
        .iter()
        .find(|r| (r.theta - 0.70).abs() < 0.01)
        .map(|r| r.regime == Regime::Tight)
        .unwrap_or(true);

    let tight_at_080 = regimes
        .iter()
        .find(|r| (r.theta - 0.80).abs() < 0.01)
        .map(|r| r.regime == Regime::Tight)
        .unwrap_or(true);

    let tight_at_085 = regimes
        .iter()
        .find(|r| (r.theta - 0.85).abs() < 0.01)
        .map(|r| r.regime == Regime::Tight)
        .unwrap_or(true);

    // Edge case: very low mean cosine distance (< 0.15) means vectors are
    // nearly identical — threshold doesn't matter much
    if mean_cosine_distance < 0.15 {
        return ThresholdRecommendation {
            semantic_threshold: DEFAULT_SEMANTIC_THRESHOLD as f64,
            rationale: "Vectors are nearly identical (d̄ < 0.15). Default threshold \
                works but corpus may be too small or homogeneous for meaningful analysis."
                .to_string(),
            adjust_weights: false,
            suggested_keyword_weight: DEFAULT_KEYWORD_WEIGHT as f64,
            suggested_semantic_weight: DEFAULT_SEMANTIC_WEIGHT as f64,
            weight_rationale: String::new(),
        };
    }

    // Case 1: TIGHT at 0.70 — geometry works well at default threshold
    if tight_at_070 {
        ThresholdRecommendation {
            semantic_threshold: DEFAULT_SEMANTIC_THRESHOLD as f64,
            rationale: "Embedding geometry is TIGHT at θ=0.70 — semantic search \
                discriminates well. Default threshold is appropriate."
                .to_string(),
            adjust_weights: false,
            suggested_keyword_weight: DEFAULT_KEYWORD_WEIGHT as f64,
            suggested_semantic_weight: DEFAULT_SEMANTIC_WEIGHT as f64,
            weight_rationale: String::new(),
        }
    }
    // Case 2: SPREAD at 0.70 but TIGHT at 0.80 — raise threshold to 0.80
    else if tight_at_080 {
        ThresholdRecommendation {
            semantic_threshold: 0.80,
            rationale: "Embedding geometry is SPREAD at θ=0.70 but TIGHT at θ=0.80. \
                Raising the threshold to 0.80 avoids false positive matches while \
                maintaining good recall."
                .to_string(),
            adjust_weights: true,
            suggested_keyword_weight: 0.5,
            suggested_semantic_weight: 0.5,
            weight_rationale: "With θ=0.80, semantic search is effective but less \
                permissive — balancing keyword and semantic equally provides \
                the best RRF fusion."
                .to_string(),
        }
    }
    // Case 3: SPREAD at all thresholds — maximize threshold, shift to keyword
    else if !tight_at_085 {
        ThresholdRecommendation {
            semantic_threshold: 0.85,
            rationale: "Embedding geometry is SPREAD at all tested thresholds. \
                Setting θ=0.85 minimizes false positives but semantic search \
                still has limited discriminative power."
                .to_string(),
            adjust_weights: true,
            suggested_keyword_weight: 0.7,
            suggested_semantic_weight: 0.3,
            weight_rationale: "With SPREAD geometry, keyword search is more \
                reliable than semantic search. Shifting weight toward keywords \
                improves retrieval quality."
                .to_string(),
        }
    }
    // Case 4: TIGHT at 0.85 but not at 0.80 — use 0.85
    else {
        ThresholdRecommendation {
            semantic_threshold: 0.85,
            rationale: "Embedding geometry is TIGHT only at θ=0.85. Using the \
                highest threshold ensures reliable discrimination."
                .to_string(),
            adjust_weights: true,
            suggested_keyword_weight: 0.6,
            suggested_semantic_weight: 0.4,
            weight_rationale: "With a high threshold, semantic search is restrictive. \
                Giving more weight to keywords broadens recall."
                .to_string(),
        }
    }
}

/// Compute variance explained at key percentiles
///
/// Reports which principal component number reaches 50%, 90%, 95%, 99%
/// of cumulative variance.
fn compute_variance_explained(eigenvalues: &[f64], _d: usize) -> VarianceExplained {
    if eigenvalues.is_empty() {
        return VarianceExplained {
            pc_50: 0,
            pc_90: 0,
            pc_95: 0,
            pc_99: 0,
        };
    }

    let total: f64 = eigenvalues.iter().sum();
    if total <= 0.0 {
        return VarianceExplained {
            pc_50: 0,
            pc_90: 0,
            pc_95: 0,
            pc_99: 0,
        };
    }

    let mut cumulative = 0.0;
    let mut pc_50 = eigenvalues.len();
    let mut pc_90 = eigenvalues.len();
    let mut pc_95 = eigenvalues.len();
    let mut pc_99 = eigenvalues.len();

    for (i, &lambda) in eigenvalues.iter().enumerate() {
        cumulative += lambda / total;
        let pc = i + 1; // 1-indexed principal component number

        if cumulative >= 0.50 && pc_50 == eigenvalues.len() {
            pc_50 = pc;
        }
        if cumulative >= 0.90 && pc_90 == eigenvalues.len() {
            pc_90 = pc;
        }
        if cumulative >= 0.95 && pc_95 == eigenvalues.len() {
            pc_95 = pc;
        }
        if cumulative >= 0.99 && pc_99 == eigenvalues.len() {
            pc_99 = pc;
        }
    }

    VarianceExplained {
        pc_50,
        pc_90,
        pc_95,
        pc_99,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Centered orthogonal vectors should have d_eff near n-1
    ///
    /// After centering, n orthogonal vectors lose 1 degree of freedom
    /// (the mean direction), so max d_eff = n-1 for centered data.
    #[test]
    fn test_d_eff_perfectly_orthogonal() {
        // Create 3 orthogonal vectors in 3D space
        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let (eigenvalues, d_eff) = compute_d_eff(&vectors, 3, 3);

        // After centering, the data has rank 2 (vectors sum to mean, lose 1 DOF)
        // So d_eff should be around 2.0, not 3.0
        assert!(
            d_eff > 1.5 && d_eff < 2.5,
            "d_eff for 3 centered orthogonal vectors should be ≈ 2 (rank = n-1), got {}",
            d_eff
        );

        // Eigenvalues should sum to approximately the total variance
        let sum: f64 = eigenvalues.iter().sum();
        assert!(
            sum > 0.1,
            "Sum of eigenvalues should be positive, got {}",
            sum
        );
    }

    /// Test: All vectors along one axis should have d_eff ≈ 1
    #[test]
    fn test_d_eff_single_direction() {
        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.99, 0.01, 0.0],
            vec![1.01, -0.01, 0.0],
            vec![0.98, 0.02, 0.0],
            vec![1.02, 0.0, 0.01],
        ];

        let (_, d_eff) = compute_d_eff(&vectors, 5, 3);

        assert!(
            d_eff < 2.0,
            "d_eff for nearly collinear vectors should be < 2.0, got {}",
            d_eff
        );
    }

    /// Test: Vectors spanning two dimensions should have d_eff ≈ 2
    ///
    /// Uses random-ish vectors that span 2D space without collinear centering.
    #[test]
    fn test_d_eff_two_dimensions() {
        // Vectors that naturally span 2D after centering
        // (no single dominant direction)
        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![1.0, 1.0, 0.0],
            vec![-1.0, 0.0, 0.0],
            vec![0.0, -1.0, 0.0],
            vec![-1.0, -1.0, 0.0],
        ];

        let (_, d_eff) = compute_d_eff(&vectors, 6, 3);

        // These span 2D, so d_eff should be around 2
        assert!(
            d_eff > 1.5 && d_eff < 3.0,
            "d_eff for 2D data should be around 2, got {}",
            d_eff
        );
    }

    /// Test: Identical vectors should have d̄ ≈ 0
    #[test]
    fn test_cosine_distance_identical() {
        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
        ];

        let (mean_cd, min_cd, max_cd) = compute_cosine_distance_stats(&vectors, 3, 3);

        assert!(
            mean_cd.abs() < 0.01,
            "Mean cosine distance for identical vectors should be ≈ 0, got {}",
            mean_cd
        );
        assert!(
            min_cd.abs() < 0.01,
            "Min cosine distance for identical vectors should be ≈ 0, got {}",
            min_cd
        );
        assert!(
            max_cd.abs() < 0.01,
            "Max cosine distance for identical vectors should be ≈ 0, got {}",
            max_cd
        );
    }

    /// Test: Orthogonal vectors should have d̄ ≈ 1
    #[test]
    fn test_cosine_distance_orthogonal() {
        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let (mean_cd, _, _) = compute_cosine_distance_stats(&vectors, 3, 3);

        assert!(
            (mean_cd - 1.0).abs() < 0.01,
            "Mean cosine distance for orthogonal vectors should be ≈ 1.0, got {}",
            mean_cd
        );
    }

    /// Test: SPREAD regime when d̄ ≥ θ'
    #[test]
    fn test_regime_classification_spread() {
        // d̄ = 0.5 — at θ=0.70, θ'=0.30, and 0.5 ≥ 0.30 → SPREAD
        let regimes = compute_regimes(0.5);
        assert_eq!(regimes[0].regime, Regime::Spread); // θ=0.70
        assert_eq!(regimes[1].regime, Regime::Spread); // θ=0.75
        assert_eq!(regimes[2].regime, Regime::Spread); // θ=0.80
        assert_eq!(regimes[3].regime, Regime::Spread); // θ=0.85
    }

    /// Test: TIGHT regime when d̄ < θ'
    #[test]
    fn test_regime_classification_tight() {
        // d̄ = 0.1 — at θ=0.70, θ'=0.30, and 0.1 < 0.30 → TIGHT
        let regimes = compute_regimes(0.1);
        assert_eq!(regimes[0].regime, Regime::Tight); // θ=0.70
        assert_eq!(regimes[1].regime, Regime::Tight); // θ=0.75
    }

    /// Test: Variance explained with known eigenvalues
    #[test]
    fn test_variance_explained() {
        // Eigenvalues in descending order: [5.0, 3.0, 1.0, 0.5, 0.5]
        // Total = 10.0
        // Cumulative: [50%, 80%, 90%, 95%, 100%]
        let eigenvalues = vec![5.0, 3.0, 1.0, 0.5, 0.5];

        let ve = compute_variance_explained(&eigenvalues, 5);

        assert_eq!(ve.pc_50, 1); // 50% at PC #1
        assert_eq!(ve.pc_90, 3); // 90% at PC #3
        assert_eq!(ve.pc_95, 4); // 95% at PC #4
        assert_eq!(ve.pc_99, 5); // 99% at PC #5
    }

    /// Test: Empty vectors should not panic
    #[test]
    fn test_empty_vectors() {
        let vectors: Vec<Vec<f64>> = vec![];
        let diagnostics = analyze_embeddings(&vectors, 256, "test-model", vec![]);

        assert_eq!(diagnostics.vector_count, 0);
        assert_eq!(diagnostics.d_eff, 0.0);
    }

    /// Test: Small corpus (n < d) should produce d_eff ≤ n-1
    ///
    /// After centering, n vectors can span at most n-1 dimensions,
    /// so d_eff should never exceed n-1.
    #[test]
    fn test_small_corpus_d_eff_unreliable() {
        // 3 vectors in 256D — after centering, max rank = 2
        let mut v1 = vec![0.0; 256];
        v1[0] = 1.0;
        let mut v2 = vec![0.0; 256];
        v2[1] = 1.0;
        let mut v3 = vec![0.0; 256];
        v3[2] = 1.0;

        let vectors: Vec<Vec<f64>> = vec![v1, v2, v3];
        let diagnostics = analyze_embeddings(&vectors, 256, "test", vec![]);

        // d_eff should be at most n-1 = 2 (not 256!)
        assert!(
            diagnostics.d_eff <= 2.5,
            "d_eff with n=3 vectors should be at most n-1=2 (with float tolerance ≤ 2.5), got {}",
            diagnostics.d_eff
        );
        assert!(
            diagnostics.d_eff >= 1.0,
            "d_eff with 3 distinct vectors should be >= 1.0, got {}",
            diagnostics.d_eff
        );
    }

    /// Test: analyze_embeddings_with_progress calls progress callback
    #[test]
    fn test_progress_callback_is_called() {
        use std::sync::{Arc, Mutex};

        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let progress_calls: Arc<Mutex<Vec<(String, f64)>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_calls = Arc::clone(&progress_calls);
        let diagnostics = analyze_embeddings_with_progress(
            &vectors,
            3,
            "test-model",
            vec![],
            &move |phase, frac| {
                captured_calls
                    .lock()
                    .unwrap()
                    .push((phase.to_string(), frac));
            },
        );

        // Should have been called at least once
        let calls = progress_calls.lock().unwrap();
        assert!(
            !calls.is_empty(),
            "Progress callback should have been called at least once"
        );

        // Should have called with known phase names
        let phases: Vec<&str> = calls.iter().map(|(p, _)| p.as_str()).collect();
        assert!(
            phases.iter().any(|p| p.contains("Centering")
                || p.contains("covariance")
                || p.contains("pairwise")),
            "Progress phases should include centering, covariance, or pairwise, got: {:?}",
            phases
        );
        drop(calls);

        // Results should be identical to analyze_embeddings (no progress)
        let baseline = analyze_embeddings(&vectors, 3, "test-model", vec![]);
        assert!(
            (diagnostics.d_eff - baseline.d_eff).abs() < 0.01,
            "d_eff with progress should match baseline: {} vs {}",
            diagnostics.d_eff,
            baseline.d_eff
        );
    }

    // ============================================================
    // Threshold recommendation tests
    // ============================================================

    /// Test: TIGHT at θ=0.70 → recommend default threshold 0.70
    #[test]
    fn test_recommend_threshold_tight_at_070() {
        // d̄ = 0.2, so at θ=0.70, θ'=0.30, and 0.2 < 0.30 → TIGHT
        let regimes = compute_regimes(0.2);
        let rec = recommend_threshold(10.0, 0.2, &regimes);
        assert!(
            (rec.semantic_threshold - 0.70).abs() < 0.01,
            "TIGHT at 0.70 should recommend θ=0.70, got {}",
            rec.semantic_threshold
        );
        assert!(
            !rec.adjust_weights,
            "Should not adjust weights when TIGHT at 0.70"
        );
    }

    /// Test: TIGHT at θ=0.70 but SPREAD at θ≥0.75 → still recommend θ=0.70
    ///
    /// With d̄=0.28: TIGHT at 0.70 (θ'=0.30), SPREAD at 0.75+ (θ'≤0.25).
    /// Because d̄ < 0.30 at θ=0.70, `tight_at_070=true` → default 0.70 is appropriate.
    #[test]
    fn test_recommend_threshold_tight_at_070_spread_above() {
        let regimes = compute_regimes(0.28);
        assert_eq!(
            regimes[0].regime,
            Regime::Tight,
            "d̄=0.28 < θ'=0.30 at θ=0.70"
        );
        assert_eq!(
            regimes[1].regime,
            Regime::Spread,
            "d̄=0.28 ≥ θ'=0.25 at θ=0.75"
        );
        assert_eq!(
            regimes[2].regime,
            Regime::Spread,
            "d̄=0.28 ≥ θ'=0.20 at θ=0.80"
        );
        assert_eq!(
            regimes[3].regime,
            Regime::Spread,
            "d̄=0.28 ≥ θ'=0.15 at θ=0.85"
        );

        let rec = recommend_threshold(10.0, 0.28, &regimes);
        assert!(
            (rec.semantic_threshold - 0.70).abs() < 0.01,
            "TIGHT at 0.70 should recommend θ=0.70, got {}",
            rec.semantic_threshold
        );
        assert!(
            !rec.adjust_weights,
            "Should not adjust weights when TIGHT at 0.70"
        );
    }

    /// Test: SPREAD at all thresholds → recommend θ=0.85 with keyword-heavy weights
    #[test]
    fn test_recommend_threshold_spread_everywhere() {
        // d̄ = 0.65 → SPREAD at all thresholds
        // θ=0.70→θ'=0.30, 0.65≥0.30→SPREAD
        // θ=0.75→θ'=0.25, 0.65≥0.25→SPREAD
        // θ=0.80→θ'=0.20, 0.65≥0.20→SPREAD
        // θ=0.85→θ'=0.15, 0.65≥0.15→SPREAD
        let regimes = compute_regimes(0.65);
        for r in &regimes {
            assert_eq!(
                r.regime,
                Regime::Spread,
                "Should be SPREAD at θ={}",
                r.theta
            );
        }

        let rec = recommend_threshold(10.0, 0.65, &regimes);
        assert!(
            (rec.semantic_threshold - 0.85).abs() < 0.01,
            "SPREAD everywhere should recommend θ=0.85, got {}",
            rec.semantic_threshold
        );
        assert!(
            rec.adjust_weights,
            "Should adjust weights for SPREAD geometry"
        );
        assert!(
            (rec.suggested_keyword_weight - 0.7).abs() < 0.01,
            "Should suggest keyword_weight=0.7, got {}",
            rec.suggested_keyword_weight
        );
        assert!(
            (rec.suggested_semantic_weight - 0.3).abs() < 0.01,
            "Should suggest semantic_weight=0.3, got {}",
            rec.suggested_semantic_weight
        );
    }

    /// Test: Very low d̄ (< 0.15) → default threshold with warning
    #[test]
    fn test_recommend_threshold_very_low_distance() {
        // d̄ = 0.05 → all TIGHT, but edge case because vectors are nearly identical
        let regimes = compute_regimes(0.05);
        let rec = recommend_threshold(10.0, 0.05, &regimes);
        assert!(
            (rec.semantic_threshold - 0.70).abs() < 0.01,
            "Very low d̄ should recommend default θ=0.70, got {}",
            rec.semantic_threshold
        );
        assert!(
            !rec.adjust_weights,
            "Should not adjust weights for very low d̄"
        );
        assert!(
            rec.rationale.contains("nearly identical"),
            "Rationale should mention low distance, got: {}",
            rec.rationale
        );
    }

    /// Test: SPREAD at 0.70-0.75, TIGHT at 0.80-0.85 → recommend θ=0.80
    #[test]
    fn test_recommend_threshold_spread_low_tight_high() {
        // d̄ = 0.22
        // θ=0.70→θ'=0.30, 0.22<0.30→TIGHT
        // θ=0.75→θ'=0.25, 0.22<0.25→TIGHT
        // θ=0.80→θ'=0.20, 0.22≥0.20→SPREAD
        // θ=0.85→θ'=0.15, 0.22≥0.15→SPREAD
        let regimes = compute_regimes(0.22);
        assert_eq!(regimes[0].regime, Regime::Tight); // 0.70
        assert_eq!(regimes[1].regime, Regime::Tight); // 0.75
        assert_eq!(regimes[2].regime, Regime::Spread); // 0.80
        assert_eq!(regimes[3].regime, Regime::Spread); // 0.85

        // TIGHT at 0.70 → default threshold 0.70, no adjustment
        let rec = recommend_threshold(10.0, 0.22, &regimes);
        assert!(
            (rec.semantic_threshold - 0.70).abs() < 0.01,
            "TIGHT at 0.70 should recommend θ=0.70, got {}",
            rec.semantic_threshold
        );
        assert!(!rec.adjust_weights);
    }

    /// Test: TIGHT at θ≤0.80 but SPREAD at θ=0.85 → default θ=0.70 still appropriate
    ///
    /// With d̄=0.18: TIGHT at 0.70/0.75/0.80, SPREAD only at 0.85.
    /// Since `tight_at_070=true`, the default threshold 0.70 is still the
    /// best choice — vectors discriminate well at this threshold.
    #[test]
    fn test_recommend_threshold_tight_low_spread_at_085() {
        let regimes = compute_regimes(0.18);
        assert_eq!(
            regimes[0].regime,
            Regime::Tight,
            "d̄=0.18 < θ'=0.30 at θ=0.70"
        );
        assert_eq!(
            regimes[1].regime,
            Regime::Tight,
            "d̄=0.18 < θ'=0.25 at θ=0.75"
        );
        assert_eq!(
            regimes[2].regime,
            Regime::Tight,
            "d̄=0.18 < θ'=0.20 at θ=0.80"
        );
        assert_eq!(
            regimes[3].regime,
            Regime::Spread,
            "d̄=0.18 ≥ θ'=0.15 at θ=0.85"
        );

        // tight_at_070=true → recommend θ=0.70 (default)
        let rec = recommend_threshold(10.0, 0.18, &regimes);
        assert!(
            (rec.semantic_threshold - 0.70).abs() < 0.01,
            "TIGHT at 0.70 should recommend θ=0.70, got {}",
            rec.semantic_threshold
        );
        assert!(
            !rec.adjust_weights,
            "Should not adjust weights when TIGHT at 0.70"
        );
    }

    /// Test: analyze_embeddings produces threshold_recommendation
    #[test]
    fn test_diagnostics_includes_threshold_recommendation() {
        let vectors: Vec<Vec<f64>> = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let diagnostics = analyze_embeddings(&vectors, 3, "test-model", vec![]);

        // Should have a threshold recommendation
        assert!(
            diagnostics.threshold_recommendation.semantic_threshold > 0.0,
            "semantic_threshold should be positive"
        );
        assert!(
            !diagnostics.threshold_recommendation.rationale.is_empty(),
            "rationale should not be empty"
        );
    }
}
