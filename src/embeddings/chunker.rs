//! Text chunking for long messages
//!
//! Splits messages into overlapping chunks for better semantic search granularity
//! using RAG-style chunking. Chunk size is determined dynamically by the
//! embedding model's context length.

use super::chunk_config::DynamicChunkConfig;

/// Default maximum chunk size in characters
#[allow(dead_code)]
pub const DEFAULT_CHUNK_SIZE: usize = 1024;

/// Default overlap between chunks (20% of chunk size)
#[allow(dead_code)]
pub const DEFAULT_CHUNK_OVERLAP: usize = 200;

/// Minimum size for the last chunk (avoid tiny chunks)
#[allow(dead_code)]
pub const DEFAULT_CHUNK_MIN_SIZE: usize = 256;

/// Configuration for text chunking
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum characters per chunk
    pub max_chars: usize,
    /// Overlap between adjacent chunks
    pub overlap_chars: usize,
    /// Minimum size for last chunk (merge with previous if smaller)
    pub min_chunk_size: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_chars: DEFAULT_CHUNK_SIZE,
            overlap_chars: DEFAULT_CHUNK_OVERLAP,
            min_chunk_size: DEFAULT_CHUNK_MIN_SIZE,
        }
    }
}

impl From<&DynamicChunkConfig> for ChunkConfig {
    fn from(config: &DynamicChunkConfig) -> Self {
        Self {
            max_chars: config.max_chars(),
            overlap_chars: config.overlap_chars(),
            min_chunk_size: config.min_chunk_chars(),
        }
    }
}

/// A single chunk of a message
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Index of this chunk (0, 1, 2, ...)
    pub index: usize,
    /// Content of the chunk
    pub content: String,
    /// Start position in original message (byte offset)
    pub start_offset: usize,
    /// End position in original message (byte offset)
    pub end_offset: usize,
}

impl Chunk {
    /// Create a new chunk
    pub fn new(index: usize, content: String, start_offset: usize, end_offset: usize) -> Self {
        Self {
            index,
            content,
            start_offset,
            end_offset,
        }
    }
}

/// Check if message needs chunking
pub fn needs_chunking(text: &str) -> bool {
    text.len() > DEFAULT_CHUNK_SIZE
}

/// Check if message needs chunking with custom config
#[expect(dead_code)]
pub fn needs_chunking_with_config(text: &str, config: &ChunkConfig) -> bool {
    text.len() > config.max_chars
}

/// Find the nearest valid UTF-8 character boundary at or before the target byte position
fn find_char_boundary(text: &str, target_byte: usize) -> usize {
    if target_byte >= text.len() {
        return text.len();
    }

    // If already on a boundary, return it
    if text.is_char_boundary(target_byte) {
        return target_byte;
    }

    // Find the previous boundary
    let mut boundary = target_byte;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

/// Find the nearest valid UTF-8 character boundary at or after the target byte position
fn find_char_boundary_forward(text: &str, target_byte: usize) -> usize {
    if target_byte >= text.len() {
        return text.len();
    }

    // If already on a boundary, return it
    if text.is_char_boundary(target_byte) {
        return target_byte;
    }

    // Find the next boundary
    let mut boundary = target_byte;
    while boundary < text.len() && !text.is_char_boundary(boundary) {
        boundary += 1;
    }
    boundary
}

/// Split text into overlapping chunks using default configuration
///
/// # Example
/// ```ignore
/// let chunks = chunk_text("very long text...");
/// assert!(chunks.len() > 1);
/// ```
pub fn chunk_text(text: &str) -> Vec<Chunk> {
    chunk_text_with_config(text, &ChunkConfig::default())
}

/// Split text into overlapping chunks with custom configuration
pub fn chunk_text_with_config(text: &str, config: &ChunkConfig) -> Vec<Chunk> {
    // If text is short enough, return as single chunk
    if text.len() <= config.max_chars {
        return vec![Chunk::new(0, text.to_string(), 0, text.len())];
    }

    let mut chunks = Vec::new();
    let text_len = text.len();
    let step = config.max_chars - config.overlap_chars;

    let mut pos = 0;
    let mut chunk_index = 0;

    while pos < text_len {
        // Calculate chunk end position
        let mut end = (pos + config.max_chars).min(text_len);

        // Adjust to valid UTF-8 boundary
        end = find_char_boundary(text, end);

        // Adjust end to sentence boundary if possible (don't break in middle of sentence)
        if end < text_len {
            end = find_sentence_boundary(text, end);
            // Ensure it's still a valid boundary after sentence adjustment
            end = find_char_boundary(text, end);
        }

        // Extract chunk content
        let chunk_content = text[pos..end].to_string();

        chunks.push(Chunk::new(chunk_index, chunk_content, pos, end));

        chunk_index += 1;

        // Move to next position with overlap
        // But ensure we advance (avoid infinite loop)
        let next_pos = end.saturating_sub(config.overlap_chars);
        // Ensure next_pos is a valid boundary
        let next_pos = find_char_boundary_forward(text, next_pos);

        if next_pos <= pos {
            // Couldn't find good boundary, advance by step
            pos += step;
            pos = find_char_boundary_forward(text, pos);
        } else {
            pos = next_pos;
        }
    }

    // Merge last chunk if it's too small
    // But only if the merged result doesn't exceed max_chars
    if chunks.len() >= 2 {
        #[expect(clippy::expect_used)] // chunks.len() >= 2 guarantees .last() returns Some
        let last_chunk = chunks
            .last()
            .expect("chunks len >= 2 guarantees at least one chunk");
        if last_chunk.content.len() < config.min_chunk_size {
            let prev_chunk = chunks[chunks.len() - 2].clone();
            let merged_len = prev_chunk.content.len() + last_chunk.content.len();

            // Only merge if the result fits within max_chars
            if merged_len <= config.max_chars {
                let merged_content = format!("{}{}", prev_chunk.content, last_chunk.content);
                let merged = Chunk::new(
                    prev_chunk.index,
                    merged_content,
                    prev_chunk.start_offset,
                    last_chunk.end_offset,
                );

                chunks.pop(); // Remove last
                chunks.pop(); // Remove previous
                chunks.push(merged); // Add merged
            }
            // If merged would exceed max_chars, keep the small chunk
            // Better to have a small chunk than one that exceeds context limit
        }
    }

    chunks
}

/// Find a good sentence boundary near the target position
///
/// Tries to find punctuation that marks end of sentence:
/// - Period followed by space and capital letter
/// - Question mark or exclamation mark
/// - Newline
///
/// Returns position after boundary, or original position if no boundary found.
fn find_sentence_boundary(text: &str, target_pos: usize) -> usize {
    // Search backwards from target_pos for sentence boundary
    let search_start = target_pos.saturating_sub(100); // Look back up to 100 chars

    // Search from target_pos backwards to find good boundary
    for pos in (search_start..target_pos).rev() {
        // Skip if not a valid UTF-8 boundary
        if !text.is_char_boundary(pos) {
            continue;
        }

        // Get the character at this position
        #[expect(clippy::expect_used)] // pos is a valid char boundary (checked above)
        let ch = text[pos..]
            .chars()
            .next()
            .expect("pos is a valid char boundary (checked above)");

        // Check for sentence-ending punctuation
        if ch == '.' || ch == '!' || ch == '?' || ch == '\n' {
            // Check if this is really end of sentence
            let next_pos = pos + ch.len_utf8();
            if next_pos >= text.len() {
                // End of text, this is a boundary
                return next_pos;
            }

            // Get next character — next_pos is pos + ch.len_utf8(), which is always a valid char boundary
            #[expect(clippy::expect_used)]
            // next_pos = pos + ch.len_utf8() is always a valid boundary
            let next_ch = text[next_pos..]
                .chars()
                .next()
                .expect("next_pos = pos + ch.len_utf8() is always a valid boundary");

            // Good boundary if followed by:
            // - Whitespace and capital letter
            // - Newline
            // - End of text
            if next_ch == '\n' {
                return next_pos;
            }

            if next_ch == ' ' || next_ch == '\t' {
                // Check if next non-whitespace is capital letter
                let lookahead = &text[next_pos..];
                let trimmed = lookahead.trim_start();
                if let Some(first_char) = trimmed.chars().next()
                    && first_char.is_uppercase()
                {
                    return next_pos;
                }
            }
        }
    }

    // No good boundary found, return original position
    target_pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_text_single_chunk() {
        let text = "Hello, world!";
        let chunks = chunk_text(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
        assert_eq!(chunks[0].start_offset, 0);
        assert_eq!(chunks[0].end_offset, text.len());
    }

    #[test]
    fn test_needs_chunking() {
        let short_text = "Hello, world!";
        let long_text = "x".repeat(2000);

        assert!(!needs_chunking(short_text));
        assert!(needs_chunking(&long_text));
    }

    #[test]
    fn test_long_text_multiple_chunks() {
        // Create text longer than DEFAULT_CHUNK_SIZE
        let text = "This is a test. ".repeat(100); // ~1800 chars
        let chunks = chunk_text(&text);

        assert!(chunks.len() > 1, "Should create multiple chunks");

        // Verify all chunks except last have content
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }

        // Verify offsets are valid
        for i in 1..chunks.len() {
            assert!(chunks[i].start_offset >= chunks[i - 1].start_offset);
        }
    }

    #[test]
    fn test_chunk_indices() {
        let text = "x".repeat(3000);
        let chunks = chunk_text(&text);

        // Verify indices are sequential
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn test_custom_config() {
        let config = ChunkConfig {
            max_chars: 100,
            overlap_chars: 20,
            min_chunk_size: 30,
        };

        let text = "x".repeat(250);
        let chunks = chunk_text_with_config(&text, &config);

        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_sentence_boundary() {
        let text = "Hello world. This is a test. Another sentence here.";

        // Find boundary near position 15
        let boundary = find_sentence_boundary(text, 15);

        // Should find the period after "world"
        assert!(boundary > 0);
    }

    #[test]
    fn test_chunk_content_coverage() {
        // Verify that chunks with overlap don't lose content
        let text = "abcdefghijklmnopqrstuvwxyz".repeat(50);
        let chunks = chunk_text(&text);

        // First chunk should start at beginning
        assert_eq!(chunks[0].start_offset, 0);

        // Last chunk should end at text end
        let last = chunks.last().unwrap();
        assert_eq!(last.end_offset, text.len());

        // Verify overlaps exist (for non-initial chunks)
        for i in 1..chunks.len() {
            assert!(
                chunks[i].start_offset < chunks[i - 1].end_offset,
                "Chunk {} should overlap with previous",
                i
            );
        }
    }

    #[test]
    fn test_empty_text() {
        let chunks = chunk_text("");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "");
    }

    #[test]
    fn test_utf8_char_boundary() {
        // Portuguese with accents - multibyte characters
        let text = "São Paulo é uma cidade maravilhosa! ".repeat(50);
        let chunks = chunk_text(&text);

        // Verify all chunks are valid UTF-8
        for chunk in &chunks {
            assert!(
                chunk.content.is_char_boundary(0),
                "Start should be boundary"
            );
            assert!(
                chunk.content.is_char_boundary(chunk.content.len()),
                "End should be boundary"
            );
        }
    }

    #[test]
    fn test_utf8_multibyte_at_boundary() {
        // Test where multibyte char would be at chunk boundary position 1024
        // 'ó' is 2 bytes (UTF-8), so position 1024 might split it
        let text = "a".repeat(1022) + "ção" + &"b".repeat(100);
        let chunks = chunk_text(&text);

        assert!(chunks.len() > 0, "Should create chunks");

        // Verify all chunks are valid UTF-8
        for chunk in &chunks {
            assert!(chunk.content.is_char_boundary(0));
            assert!(chunk.content.is_char_boundary(chunk.content.len()));
        }
    }

    #[test]
    fn test_emoji_in_text() {
        // Emojis are 4 bytes in UTF-8
        let text = "Hello 👋 World 🌍 test! ".repeat(100);
        let chunks = chunk_text(&text);

        for chunk in &chunks {
            assert!(chunk.content.is_char_boundary(0));
            assert!(chunk.content.is_char_boundary(chunk.content.len()));
        }
    }

    #[test]
    fn test_chinese_characters() {
        // Chinese characters are 3 bytes in UTF-8
        let text = "你好世界测试中文 ".repeat(100);
        let chunks = chunk_text(&text);

        for chunk in &chunks {
            assert!(chunk.content.is_char_boundary(0));
            assert!(chunk.content.is_char_boundary(chunk.content.len()));
        }
    }
}
