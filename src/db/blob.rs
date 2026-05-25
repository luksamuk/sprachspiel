//! Blob deserialization utilities for sqlite-vec vector embeddings
//!
//! sqlite-vec stores FLOAT vectors as raw little-endian f32 bytes (BLOBs).
//! This module provides the `blob_to_f32_vec` function to deserialize these
//! BLOBs back into `Vec<f32>`, shared by content and fact embedding reads.

/// Deserialize a BLOB (raw f32 bytes) into a Vec<f32>
///
/// sqlite-vec stores FLOAT vectors as raw little-endian f32 bytes.
/// Each f32 is 4 bytes, so the blob length must be a multiple of 4.
/// Any trailing bytes that don't form a complete f32 are silently ignored
/// (this matches sqlite-vec behavior where vector dimensions are declared
/// in the schema).
pub fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
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
        // f32 1.0 + f32 2.0 + f32 3.0
        let mut blob = Vec::new();
        blob.extend_from_slice(&1.0f32.to_ne_bytes());
        blob.extend_from_slice(&2.0f32.to_ne_bytes());
        blob.extend_from_slice(&3.0f32.to_ne_bytes());

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
        // 8 bytes = 2 f32 values, plus 3 trailing bytes (incomplete)
        let mut blob = Vec::new();
        blob.extend_from_slice(&1.0f32.to_ne_bytes());
        blob.extend_from_slice(&2.0f32.to_ne_bytes());
        blob.extend_from_slice(&[0xAB, 0xCD, 0xEF]); // trailing 3 bytes

        let result = blob_to_f32_vec(&blob);
        assert_eq!(result.len(), 2); // trailing bytes ignored
        assert!((result[0] - 1.0f32).abs() < f32::EPSILON);
        assert!((result[1] - 2.0f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blob_to_f32_vec_negative_values() {
        let mut blob = Vec::new();
        blob.extend_from_slice(&(-1.0f32).to_ne_bytes());
        blob.extend_from_slice(&(-0.5f32).to_ne_bytes());

        let result = blob_to_f32_vec(&blob);
        assert_eq!(result.len(), 2);
        assert!((result[0] - (-1.0f32)).abs() < f32::EPSILON);
        assert!((result[1] - (-0.5f32)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blob_to_f32_vec_256_dimensions() {
        // Simulate a 256-dimension embedding (the standard truncated size)
        let original: Vec<f32> = (0..256).map(|i| i as f32 * 0.01).collect();
        let mut blob = Vec::new();
        for val in &original {
            blob.extend_from_slice(&val.to_ne_bytes());
        }

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
        // Create a known vector, serialize to bytes, deserialize back
        let original: Vec<f32> = vec![0.123, -0.456, 0.789, -0.012, 3.14159];
        let blob: Vec<u8> = original.iter().flat_map(|f| f.to_ne_bytes()).collect();
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
}
