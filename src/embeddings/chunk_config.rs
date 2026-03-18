//! Dynamic chunk configuration based on embedding model context length.
//!
//! Different embedding models have different context length limits.
//! This module provides dynamic chunk sizing based on the model's capabilities.

/// Default percentage of context to use for chunk content (leaves margin for safety)
const DEFAULT_CHUNK_PERCENT: f32 = 0.90;

/// Default overlap between chunks (percentage of chunk size)
const DEFAULT_OVERLAP_PERCENT: f32 = 0.20;

/// Default minimum chunk size (percentage of chunk size)
const DEFAULT_MIN_CHUNK_PERCENT: f32 = 0.25;

/// Token margin for prefix (e.g., "search_document: " is ~20 tokens)
const DEFAULT_PREFIX_MARGIN: usize = 30;

/// Characters per token ratio (conservative for Portuguese/code)
/// English is ~4 chars/token, Portuguese/code is ~3 chars/token
const DEFAULT_CHARS_PER_TOKEN: f32 = 3.0;

/// Dynamic chunk configuration based on model context length.
///
/// Calculates appropriate chunk sizes based on the embedding model's context
/// length limit. Different models have vastly different limits:
/// - nomic-embed-text-v2-moe: 512 tokens
/// - nomic-embed-text-v1.5: 8192 tokens
/// - text-embedding-ada-002: 8192 tokens
#[derive(Debug, Clone)]
pub struct DynamicChunkConfig {
    /// Context length from model (e.g., 512 for nomic-embed-text-v2-moe)
    context_length: usize,
    /// Maximum chunk as percentage of available context (0.0-1.0)
    chunk_percent: f32,
    /// Overlap as percentage of chunk size (0.0-1.0)
    overlap_percent: f32,
    /// Minimum chunk as percentage of chunk size (0.0-1.0)
    min_chunk_percent: f32,
    /// Token margin for prefix (default: 30)
    prefix_margin: usize,
    /// Characters per token ratio (default: 3.0)
    chars_per_token: f32,
}

impl DynamicChunkConfig {
    /// Create with default percentages for given context length.
    pub fn new(context_length: usize) -> Self {
        Self {
            context_length,
            chunk_percent: DEFAULT_CHUNK_PERCENT,
            overlap_percent: DEFAULT_OVERLAP_PERCENT,
            min_chunk_percent: DEFAULT_MIN_CHUNK_PERCENT,
            prefix_margin: DEFAULT_PREFIX_MARGIN,
            chars_per_token: DEFAULT_CHARS_PER_TOKEN,
        }
    }

    /// Calculate maximum chunk size in characters.
    ///
    /// Formula: (context_length - prefix_margin) × chunk_percent × chars_per_token
    ///
    /// Example for nomic-embed-text-v2-moe (512 tokens):
    /// (512 - 30) × 0.90 × 3.0 = 1305 characters
    pub fn max_chars(&self) -> usize {
        let available_tokens = self.context_length.saturating_sub(self.prefix_margin);
        let chunk_tokens = available_tokens as f32 * self.chunk_percent;
        (chunk_tokens * self.chars_per_token) as usize
    }

    /// Calculate overlap between chunks in characters.
    pub fn overlap_chars(&self) -> usize {
        (self.max_chars() as f32 * self.overlap_percent) as usize
    }

    /// Calculate minimum chunk size in characters.
    pub fn min_chunk_chars(&self) -> usize {
        (self.max_chars() as f32 * self.min_chunk_percent) as usize
    }
}

impl Default for DynamicChunkConfig {
    fn default() -> Self {
        // Default to conservative 512 token context
        Self::new(512)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DynamicChunkConfig::default();
        // (512 - 30) * 0.90 * 3.0 = 482 * 0.90 * 3.0 = 1301.4 → 1301
        assert_eq!(config.max_chars(), 1301);
        assert_eq!(config.overlap_chars(), 260); // 1301 * 0.20
        assert_eq!(config.min_chunk_chars(), 325); // 1301 * 0.25
    }

    #[test]
    fn test_nomic_v2_moe() {
        let config = DynamicChunkConfig::new(512);
        assert_eq!(config.max_chars(), 1301);
        assert!(config.overlap_chars() > 0);
    }

    #[test]
    fn test_nomic_v1_5() {
        // v1.5 has 8192 token context
        let config = DynamicChunkConfig::new(8192);
        // (8192 - 30) * 0.90 * 3.0 = 8162 * 0.90 * 3.0 = 22037.4 → 22037
        assert_eq!(config.max_chars(), 22037);
    }

    #[test]
    fn test_small_context() {
        // Very small context should still work
        let config = DynamicChunkConfig::new(100);
        // (100 - 30) * 0.90 * 3.0 = 70 * 0.90 * 3.0 = 189 → 189
        assert!(config.max_chars() > 0);
        assert_eq!(config.max_chars(), 189);
    }
}
