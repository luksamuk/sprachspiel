//! Dynamic chunk configuration based on embedding model context length.
//!
//! Different embedding models have different context length limits.
//! This module provides dynamic chunk sizing based on the model's capabilities.

/// Default percentage of context to use for chunk content (leaves margin for safety)
///
/// Reduced from 0.90 to 0.80 to provide wider safety margin since token counts
/// are estimated (chars/3.0) rather than exact. The embedding API returns
/// `prompt_eval_count` but ollama-rs v0.3.4 doesn't capture it.
const DEFAULT_CHUNK_PERCENT: f32 = 0.80;

/// Default overlap between chunks (percentage of chunk size)
const DEFAULT_OVERLAP_PERCENT: f32 = 0.20;

/// Default minimum chunk size (percentage of chunk size)
const DEFAULT_MIN_CHUNK_PERCENT: f32 = 0.25;

/// Token margin for prefix (e.g., "search_document: " is ~20 tokens + 20 overhead)
///
/// Increased from 30 to 40 to account for tokenizer overhead and prefix formatting.
/// When exact token counts become available (via reqwest-based provider), this can
/// be reduced back to 30 or replaced with actual prefix token counts.
const DEFAULT_PREFIX_MARGIN: usize = 40;

/// Characters per token ratio (conservative for Portuguese/code)
///
/// This is an estimate. The Ollama embedding API returns `prompt_eval_count`
/// (exact value) but the ollama-rs library ignores it. When provider abstraction
/// is implemented with direct reqwest calls, we can use exact counts instead.
///
/// English averages ~4 chars/token, Portuguese/code averages ~3 chars/token.
/// We use the more conservative (lower) ratio to overestimate token count.
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

    /// Get the context length used by this config.
    #[allow(dead_code)]
    pub fn context_length(&self) -> usize {
        self.context_length
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
        // (512 - 40) * 0.80 * 3.0 = 472 * 0.80 * 3.0 = 1132.8 → 1132
        assert_eq!(config.max_chars(), 1132);
        assert_eq!(config.overlap_chars(), 226); // 1132 * 0.20 = 226.4 → 226
        assert_eq!(config.min_chunk_chars(), 283); // 1132 * 0.25 = 283
    }

    #[test]
    fn test_nomic_v2_moe() {
        let config = DynamicChunkConfig::new(512);
        assert_eq!(config.max_chars(), 1132);
        assert!(config.overlap_chars() > 0);
    }

    #[test]
    fn test_nomic_v1_5() {
        // v1.5 has 8192 token context
        let config = DynamicChunkConfig::new(8192);
        // (8192 - 40) * 0.80 * 3.0 = 8152 * 0.80 * 3.0 = 19564.8 → 19564
        assert_eq!(config.max_chars(), 19564);
    }

    #[test]
    fn test_small_context() {
        // Very small context should still work
        let config = DynamicChunkConfig::new(100);
        // (100 - 40) * 0.80 * 3.0 = 60 * 0.80 * 3.0 = 144
        assert!(config.max_chars() > 0);
        assert_eq!(config.max_chars(), 144);
    }

    #[test]
    fn test_context_length() {
        let config = DynamicChunkConfig::new(512);
        assert_eq!(config.context_length(), 512);

        let config2 = DynamicChunkConfig::new(256);
        assert_eq!(config2.context_length(), 256);
    }

    #[test]
    fn test_halving_progression() {
        // Test that halving works by creating configs manually
        let ctx = 512;

        // Full context
        let c1 = DynamicChunkConfig::new(ctx);
        assert_eq!(c1.max_chars(), 1132); // 512 tokens

        // Halved: 256 tokens
        let c2 = DynamicChunkConfig::new(ctx / 2);
        // (256 - 40) * 0.80 * 3.0 = 216 * 0.80 * 3.0 = 518.4 → 518
        assert_eq!(c2.max_chars(), 518);

        // Quarter: 128 tokens
        let c3 = DynamicChunkConfig::new(ctx / 4);
        // (128 - 40) * 0.80 * 3.0 = 88 * 0.80 * 3.0 = 211.2 → 211
        assert_eq!(c3.max_chars(), 211);

        // Eighth: 64 tokens
        let c4 = DynamicChunkConfig::new(ctx / 8);
        // (64 - 40) * 0.80 * 3.0 = 24 * 0.80 * 3.0 = 57.6 → 57
        assert_eq!(c4.max_chars(), 57);
    }
}
