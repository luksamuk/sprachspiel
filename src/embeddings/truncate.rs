#![expect(clippy::print_stderr)] // Embedding truncation output

//! Matryoshka embedding truncation
//!
//! Truncates 768-dimensional embeddings to 256 dimensions with normalization.
//! This reduces storage by 3x with only 2-3% quality loss.

/// Full embedding dimensions from nomic-embed-text-v2-moe
pub const FULL_DIMENSIONS: usize = 768;
/// Truncated dimensions (Matryoshka)
pub const TRUNCATED_DIMENSIONS: usize = 256;

/// Truncate and normalize a 768-dimensional embedding to 256 dimensions.
///
/// Matryoshka embeddings allow truncation to smaller dimensions while
/// preserving most of the semantic information.
///
/// # Arguments
/// * `embedding` - Full 768-dimensional embedding
///
/// # Returns
/// * Truncated and L2-normalized 256-dimensional vector
///
/// # Panics
/// * If embedding has fewer than 256 dimensions
#[expect(clippy::panic)] // invariant: embedding must have >= TRUNCATED_DIMENSIONS, documented in # Panics
pub fn truncate_and_normalize(embedding: &[f32]) -> Vec<f32> {
    if embedding.len() < TRUNCATED_DIMENSIONS {
        panic!(
            "Embedding too short: expected at least {} dimensions, got {}",
            TRUNCATED_DIMENSIONS,
            embedding.len()
        );
    }

    // Warn if not using full dimensions (for quality consistency)
    if embedding.len() != FULL_DIMENSIONS && cfg!(debug_assertions) {
        eprintln!(
            "Warning: Embedding has {} dimensions, expected {}",
            embedding.len(),
            FULL_DIMENSIONS
        );
    }

    // Take first 256 dimensions
    let truncated = &embedding[..TRUNCATED_DIMENSIONS];

    // L2 normalize
    let norm: f32 = truncated.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm < f32::EPSILON {
        // Return zeros if embedding is degenerate
        return vec![0.0; TRUNCATED_DIMENSIONS];
    }

    truncated.iter().map(|x| x / norm).collect()
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

        let truncated = truncate_and_normalize(&embedding);

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
        truncate_and_normalize(&embedding);
    }
}
