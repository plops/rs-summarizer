use crate::errors::VizError;

/// Convert a raw byte blob (little-endian f32s) to a Vec<f32>.
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Convert a Vec<f32> embedding to raw bytes (little-endian).
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert a raw byte blob to a Vec<f32> with truncation validation.
/// Checks that the blob length is a multiple of 4 and at least `dim * 4` bytes.
/// Returns the first `dim` f32 values from the embedding.
pub fn bytes_to_embedding_truncated(bytes: &[u8], dim: usize) -> Result<Vec<f32>, VizError> {
    // Check if length is a multiple of 4
    if bytes.len() % 4 != 0 {
        return Err(VizError::InvalidBlobLength { length: bytes.len() });
    }

    // Check if we have enough bytes for the requested dimension
    let required_bytes = dim * 4;
    if bytes.len() < required_bytes {
        return Err(VizError::BlobTooShort { 
            actual: bytes.len(), 
            required: required_bytes 
        });
    }

    // Convert to embedding and truncate to requested dimension
    let embedding = bytes_to_embedding(bytes);
    Ok(embedding[..dim].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_bytes_to_embedding_empty() {
        let bytes: Vec<u8> = vec![];
        let result = bytes_to_embedding(&bytes);
        assert!(result.is_empty());
    }

    #[test]
    fn test_invalid_blob_length() {
        let bytes = vec![1, 2, 3]; // Not a multiple of 4
        let result = bytes_to_embedding_truncated(&bytes, 1);
        assert!(matches!(result, Err(VizError::InvalidBlobLength { length: 3 })));
    }

    #[test]
    fn test_blob_too_short() {
        let bytes = vec![1, 2, 3, 4]; // 4 bytes, only enough for 1 f32
        let result = bytes_to_embedding_truncated(&bytes, 2); // Need 8 bytes for 2 f32
        assert!(matches!(result, Err(VizError::BlobTooShort { actual: 4, required: 8 })));
    }

    #[test]
    fn test_bytes_to_embedding_truncated_success() {
        // Create embedding: [1.0, 2.0, 3.0, 4.0]
        let embedding = vec![1.0f32, 2.0, 3.0, 4.0];
        let bytes = embedding_to_bytes(&embedding);
        
        // Truncate to first 2 elements
        let result = bytes_to_embedding_truncated(&bytes, 2).unwrap();
        assert_eq!(result, vec![1.0, 2.0]);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        // Property 1: Embedding Round-Trip
        #[test]
        fn prop_embedding_round_trip(values in prop::collection::vec(prop::num::f32::NORMAL, 1..=3072)) {
            let bytes = embedding_to_bytes(&values);
            let recovered = bytes_to_embedding(&bytes);
            prop_assert_eq!(values, recovered);
        }

        // Property 2: Embedding Truncation
        #[test]
        fn prop_embedding_truncation(
            values in prop::collection::vec(prop::num::f32::NORMAL, 768..=3072),
            dim in 1..=768usize
        ) {
            // Ensure we have enough values for the requested dimension
            prop_assume!(values.len() >= dim);
            
            let bytes = embedding_to_bytes(&values);
            let result = bytes_to_embedding_truncated(&bytes, dim).unwrap();
            
            // Check length matches requested dimension
            prop_assert_eq!(result.len(), dim);
            
            // Check first dim elements are bit-exact
            prop_assert_eq!(result, values[..dim].to_vec());
        }
    }
}
