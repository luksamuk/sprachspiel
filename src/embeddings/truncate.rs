//! Matryoshka embedding truncation
//!
//! Truncates 768-dimensional embeddings to 256 dimensions with normalization.
//! This reduces storage by 3x with only 2-3% quality loss.
//!
//! # Norm Correction
//!
//! When truncating from 768 to 256 dimensions, the L2 norm of the truncated
//! vector differs from 1.0 even after normalization. The `norm_correction`
//! factor (computed as `1/(norm²)`) allows correcting cosine similarity
//! scores at query time: `true_cosine ≈ measured_cosine * sqrt(nc_query * nc_result)`.
//!
//! For nomic-embed-text-v2-moe with d_eff≈7, this correction can shift
//! similarity scores by 10-30%, making semantic search thresholds more accurate.

/// Full embedding dimensions from nomic-embed-text-v2-moe (test reference)
#[cfg(test)]
pub const FULL_DIMENSIONS: usize = 768;
/// Truncated dimensions (Matryoshka)
pub const TRUNCATED_DIMENSIONS: usize = 256;

/// Result of Matryoshka truncation with norm correction.
///
/// Contains the normalized 256-dim vector and a correction factor
/// that compensates for norm loss during truncation.
#[derive(Debug, Clone)]
pub struct TruncateResult {
    /// L2-normalized truncated 256-dim vector
    pub vector: Vec<f32>,
    /// Norm correction factor: `1 / (L2_norm_of_truncated_dims²)`.
    ///
    /// At query time, multiply cosine similarity by `sqrt(nc_query * nc_result)`
    /// to correct for the truncation-induced norm bias.
    /// A value of 1.0 means no correction needed (degenerate/zero vector).
    pub norm_correction: f32,
}

/// Truncate and normalize a 768-dimensional embedding to 256 dimensions.
///
/// Convenience wrapper around [`truncate_and_normalize_with_correction`] that
/// discards the norm correction factor. Production code should use the
/// `_with_correction` variant when storing embeddings to database for accurate
/// cosine similarity at query time.
///
/// # Arguments
/// * `embedding` - Full 768-dimensional embedding
///
/// # Returns
/// * Truncated and L2-normalized 256-dimensional vector
///
/// # Panics
/// * If embedding has fewer than 256 dimensions
#[cfg(test)]
pub fn truncate_and_normalize(embedding: &[f32], target_dims: usize) -> Vec<f32> {
    truncate_and_normalize_with_correction(embedding, target_dims).vector
}

/// Truncate, normalize, and compute norm correction for an embedding.
///
/// Truncates the embedding to `target_dims` dimensions (Matryoshka),
/// L2-normalizes the truncated vector, and computes a norm correction
/// factor for accurate cosine similarity at query time.
///
/// # Arguments
/// * `embedding` - Full embedding vector (must have >= `target_dims` elements)
/// * `target_dims` - Number of dimensions to truncate to
///
/// # Returns
/// * [`TruncateResult`] with normalized vector and norm correction
///
/// # Panics
/// * If embedding has fewer than `target_dims` dimensions
#[expect(clippy::panic)] // invariant: embedding must have >= target_dims, documented in # Panics
pub fn truncate_and_normalize_with_correction(
    embedding: &[f32],
    target_dims: usize,
) -> TruncateResult {
    if embedding.len() < target_dims {
        panic!(
            "Embedding too short: expected at least {} dimensions, got {}",
            target_dims,
            embedding.len()
        );
    }

    // Take first target_dims dimensions (Matryoshka truncation)
    let truncated = &embedding[..target_dims];

    // L2 normalize
    let norm: f32 = truncated.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm < f32::EPSILON {
        // Return zeros if embedding is degenerate
        return TruncateResult {
            vector: vec![0.0; target_dims],
            norm_correction: 1.0, // No correction for degenerate vectors
        };
    }

    // Norm correction: 1/(norm²) where norm is the L2 norm of truncated dims
    // This corrects for the fact that truncation loses dimensions that
    // contributed to the original L2 norm.
    let norm_correction = 1.0 / (norm * norm);

    let vector: Vec<f32> = truncated.iter().map(|x| x / norm).collect();

    TruncateResult {
        vector,
        norm_correction,
    }
}

/// Calculate cosine similarity between two normalized vectors.
///
/// Used by fact verification (`facts::verify`) for deduplication
/// and semantic similarity comparison.
///
/// # Panics
///
/// Panics if vectors have different lengths.
#[expect(clippy::panic)] // invariant: vectors must have equal length, programming error otherwise
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        panic!("Vector length mismatch: {} vs {}", a.len(), b.len());
    }

    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_and_normalize() {
        // Create a 768-dimensional embedding
        let embedding: Vec<f32> = (0..768).map(|i| (i % 10) as f32 / 10.0).collect();

        let truncated = truncate_and_normalize(&embedding, TRUNCATED_DIMENSIONS);

        assert_eq!(truncated.len(), TRUNCATED_DIMENSIONS);

        // Check L2 normalization
        let norm: f32 = truncated.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.0001,
            "Norm should be ~1.0, got {}",
            norm
        );
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.0001);
    }

    #[test]
    #[should_panic(expected = "Embedding too short")]
    fn test_truncate_too_short() {
        let embedding = vec![1.0, 2.0, 3.0]; // Only 3 dimensions
        truncate_and_normalize(&embedding, TRUNCATED_DIMENSIONS);
    }

    #[test]
    fn test_truncate_with_correction_basic() {
        // Create a 768-dimensional embedding with known norm
        let embedding: Vec<f32> = (0..768).map(|i| (i % 10) as f32 / 10.0).collect();

        let result = truncate_and_normalize_with_correction(&embedding, TRUNCATED_DIMENSIONS);

        assert_eq!(result.vector.len(), TRUNCATED_DIMENSIONS);

        // Check L2 normalization of the vector
        let norm: f32 = result.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.0001,
            "Norm should be ~1.0, got {}",
            norm
        );

        // Norm correction should be positive and > 0
        assert!(
            result.norm_correction > 0.0,
            "Norm correction should be positive, got {}",
            result.norm_correction
        );
    }

    #[test]
    fn test_truncate_with_correction_unit_vector() {
        // A unit vector in the first 256 dimensions should have norm_correction ≈ 1.0
        let mut embedding = vec![0.0f32; 768];
        embedding[0] = 1.0; // Unit vector along first dimension

        let result = truncate_and_normalize_with_correction(&embedding, TRUNCATED_DIMENSIONS);

        // For a unit vector, truncated norm = 1.0, so norm_correction = 1/(1²) = 1.0
        assert!(
            (result.norm_correction - 1.0).abs() < 0.0001,
            "Norm correction for unit vector should be ~1.0, got {}",
            result.norm_correction
        );
    }

    #[test]
    fn test_truncate_with_correction_degenerate() {
        // Zero vector should return norm_correction = 1.0 (no correction)
        let embedding = vec![0.0f32; 768];

        let result = truncate_and_normalize_with_correction(&embedding, TRUNCATED_DIMENSIONS);

        assert_eq!(result.vector.len(), TRUNCATED_DIMENSIONS);
        assert!(
            (result.norm_correction - 1.0).abs() < f32::EPSILON,
            "Degenerate vector should have norm_correction=1.0, got {}",
            result.norm_correction
        );
        // All elements should be 0.0
        for val in &result.vector {
            assert!(val.abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_norm_correction_formula() {
        // For a specific embedding, verify the norm correction formula:
        // norm_correction = 1/(norm²) where norm = L2 norm of truncated dims
        let mut embedding = vec![0.0f32; 768];
        embedding[0] = 0.6;
        embedding[1] = 0.8;
        // Remaining truncated dims are 0.0
        // Truncated norm = sqrt(0.36 + 0.64) = sqrt(1.0) = 1.0
        // norm_correction = 1/1.0 = 1.0

        let result = truncate_and_normalize_with_correction(&embedding, TRUNCATED_DIMENSIONS);
        assert!(
            (result.norm_correction - 1.0).abs() < 0.0001,
            "Expected norm_correction ≈ 1.0, got {}",
            result.norm_correction
        );
    }

    #[test]
    fn test_truncate_to_384_dims() {
        let embedding: Vec<f32> = (0..768).map(|i| (i % 10) as f32 / 10.0).collect();
        let result = truncate_and_normalize_with_correction(&embedding, 384);
        assert_eq!(result.vector.len(), 384);
        let norm: f32 = result.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
        assert!(result.norm_correction > 0.0);
    }
}
