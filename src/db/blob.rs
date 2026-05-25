//! Blob serialization utilities for sqlite-vec vector embeddings
//!
//! sqlite-vec stores FLOAT vectors as raw little-endian f32 bytes (BLOBs).
//! This module provides symmetric read/write functions that consistently use
//! little-endian byte order, ensuring cross-platform database portability.
//!
//! # Endianness Contract
//!
//! - **Write path:** `embedding_to_le_bytes()` serializes each f32 as 4 LE bytes.
//! - **Read path:** `blob_to_f32_vec()` deserializes 4-byte chunks as LE f32.
//!
//! Both functions use `f32::to_le_bytes()` / `f32::from_le_bytes()` explicitly,
//! not `to_ne_bytes()` / `from_ne_bytes()`. This guarantees that databases
//! written on x86_64 (native LE) can be read on any architecture, including
//! big-endian systems.

/// Serialize a slice of f32 embedding values into a little-endian byte vector
///
/// Produces a contiguous BLOB suitable for sqlite-vec FLOAT[dimensions] columns.
/// Each f32 is encoded as 4 bytes in little-endian order.
///
/// # Arguments
/// * `embedding` - Slice of f32 values to serialize
///
/// # Returns
/// A `Vec<u8>` of length `embedding.len() * 4`
pub fn embedding_to_le_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize a BLOB (raw f32 bytes) into a Vec<f32>
///
/// sqlite-vec stores FLOAT vectors as raw little-endian f32 bytes.
/// Each f32 is 4 bytes, so the blob length must be a multiple of 4.
/// Any trailing bytes that don't form a complete f32 are silently ignored
/// (this matches sqlite-vec behavior where vector dimensions are declared
/// in the schema).
///
/// # Endianness
///
/// Uses `f32::from_le_bytes()` for cross-platform compatibility. The write
/// path (`embedding_to_le_bytes()`) uses `f32::to_le_bytes()`, ensuring
/// round-trip consistency regardless of host architecture.
pub fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_to_f32_vec_single_value() {
        // f32 1.0 in little-endian: 0x3F800000 → [0x00, 0x00, 0x80, 0x3F]
        let blob: Vec<u8> = vec![0x00, 0x00, 0x80, 0x3F];
        let result = blob_to_f32_vec(&blob);
        assert_eq!(result.len(), 1);
        assert!((result[0] - 1.0f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blob_to_f32_vec_multiple_values() {
        // Encode via embedding_to_le_bytes for guaranteed LE roundtrip
        let original: Vec<f32> = vec![1.0, 2.0, 3.0];
        let blob = embedding_to_le_bytes(&original);
        let result = blob_to_f32_vec(&blob);

        assert_eq!(result.len(), 3);
        assert!((result[0] - 1.0f32).abs() < f32::EPSILON);
        assert!((result[1] - 2.0f32).abs() < f32::EPSILON);
        assert!((result[2] - 3.0f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blob_to_f32_vec_empty() {
        let blob: Vec<u8> = vec![];
        let result = blob_to_f32_vec(&blob);
        assert!(result.is_empty());
    }

    #[test]
    fn test_blob_to_f32_vec_trailing_bytes_ignored() {
        let mut blob = embedding_to_le_bytes(&[1.0, 2.0]);
        blob.extend_from_slice(&[0xAB, 0xCD, 0xEF]); // trailing 3 bytes

        let result = blob_to_f32_vec(&blob);
        assert_eq!(result.len(), 2); // trailing bytes ignored
        assert!((result[0] - 1.0f32).abs() < f32::EPSILON);
        assert!((result[1] - 2.0f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blob_to_f32_vec_negative_values() {
        let original: Vec<f32> = vec![-1.0, -0.5];
        let blob = embedding_to_le_bytes(&original);
        let result = blob_to_f32_vec(&blob);

        assert_eq!(result.len(), 2);
        assert!((result[0] - (-1.0f32)).abs() < f32::EPSILON);
        assert!((result[1] - (-0.5f32)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blob_to_f32_vec_256_dimensions() {
        // Simulate a 256-dimension embedding (the standard truncated size)
        let original: Vec<f32> = (0..256).map(|i| i as f32 * 0.01).collect();
        let blob = embedding_to_le_bytes(&original);
        let result = blob_to_f32_vec(&blob);

        assert_eq!(result.len(), 256);
        for (i, val) in result.iter().enumerate() {
            assert!(
                (val - original[i]).abs() < f32::EPSILON,
                "Mismatch at index {}: got {}, expected {}",
                i,
                val,
                original[i]
            );
        }
    }

    #[test]
    fn test_blob_to_f32_vec_roundtrip() {
        // Create a known vector, serialize to LE bytes, deserialize back
        let original: Vec<f32> = vec![0.123, -0.456, 0.789, -0.012, 3.14159];
        let blob = embedding_to_le_bytes(&original);
        let result = blob_to_f32_vec(&blob);

        assert_eq!(result.len(), original.len());
        for (i, (got, expected)) in result.iter().zip(original.iter()).enumerate() {
            assert!(
                (got - expected).abs() < f32::EPSILON,
                "Roundtrip mismatch at index {}: got {}, expected {}",
                i,
                got,
                expected
            );
        }
    }

    #[test]
    fn test_embedding_to_le_bytes_roundtrip() {
        let original: Vec<f32> = vec![0.0, 1.0, -1.0, 0.5, f32::MIN, f32::MAX];
        let blob = embedding_to_le_bytes(&original);

        // Verify blob length
        assert_eq!(blob.len(), original.len() * 4);

        // Verify roundtrip
        let result = blob_to_f32_vec(&blob);
        assert_eq!(result.len(), original.len());
        for (i, (got, expected)) in result.iter().zip(original.iter()).enumerate() {
            assert!(
                (got - expected).abs() < f32::EPSILON,
                "LE roundtrip mismatch at index {}: got {}, expected {}",
                i,
                got,
                expected
            );
        }
    }
}
