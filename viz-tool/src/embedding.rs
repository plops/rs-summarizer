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
        return Err(VizError::InvalidBlobLength {
            length: bytes.len(),
        });
    }

    // Check if we have enough bytes for the requested dimension
    let required_bytes = dim * 4;
    if bytes.len() < required_bytes {
        return Err(VizError::BlobTooShort {
            actual: bytes.len(),
            required: required_bytes,
        });
    }

    // Convert to embedding and truncate to requested dimension
    let embedding = bytes_to_embedding(bytes);
    Ok(embedding[..dim].to_vec())
}
