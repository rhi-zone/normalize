//! Embedding generation via fastembed (ONNX-backed, no server required).
//!
//! The embedder wraps a fastembed `TextEmbedding` model and serializes/
//! deserializes raw f32 vectors for storage in SQLite BLOBs.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::Path;

/// Default embedding model — nomic-embed-text-v1.5 gives 768 dimensions and
/// good code+text mixed-domain performance with Matryoshka support.
pub const DEFAULT_MODEL: &str = "nomic-embed-text-v1.5";

/// Wraps the fastembed model and provides encode/decode helpers.
pub struct Embedder {
    model: TextEmbedding,
    pub model_name: String,
    pub dimensions: usize,
}

impl Embedder {
    /// Load the model, downloading it if necessary.
    ///
    /// `cache_dir` is the directory used for ONNX model caching (typically
    /// `~/.cache/huggingface` or similar); if `None` fastembed uses its default.
    pub fn load(model_name: &str, cache_dir: Option<&Path>) -> anyhow::Result<Self> {
        let embedding_model = resolve_model(model_name)?;
        let mut opts = InitOptions::new(embedding_model);
        if let Some(dir) = cache_dir {
            opts = opts.with_cache_dir(dir.to_path_buf());
        }
        let mut model = TextEmbedding::try_new(opts).map_err(|e| {
            anyhow::anyhow!("Failed to load embedding model '{}': {}", model_name, e)
        })?;

        // Probe dimensions by embedding an empty string.
        let probe = model
            .embed(vec![""], None)
            .map_err(|e| anyhow::anyhow!("Failed to probe embedding dimensions: {}", e))?;
        let dimensions = probe.first().map(|v| v.len()).unwrap_or(768);

        Ok(Self {
            model,
            model_name: model_name.to_string(),
            dimensions,
        })
    }

    /// Embed a batch of texts. Returns one vector per input, in order.
    pub fn embed_batch(&mut self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        self.model
            .embed(texts, None)
            .map_err(|e| anyhow::anyhow!("Embedding failed: {}", e))
    }

    /// Embed a single text.
    pub fn embed_one(&mut self, text: &str) -> anyhow::Result<Vec<f32>> {
        let batch = self.embed_batch(&[text])?;
        batch
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Embedder returned empty result for single text"))
    }
}

/// Convert a slice of f32 to a little-endian byte blob for SQLite storage.
pub fn encode_vector(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &x in v {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    bytes
}

/// Decode a little-endian byte blob back to f32 values.
pub fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap_or([0u8; 4])))
        .collect()
}

/// Cosine similarity between two equal-length vectors.
/// Returns 0.0 if either vector has zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(
        a.len(),
        b.len(),
        "cosine_similarity: vector length mismatch"
    );
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    (dot / (mag_a * mag_b)).clamp(-1.0, 1.0)
}

/// Return the known output dimensionality for a model without loading it.
///
/// Returns `None` for unknown models (caller should default to 768 or probe at
/// load time via [`Embedder::dimensions`]).
pub fn dims_for_model(name: &str) -> Option<usize> {
    match name {
        "nomic-embed-text-v1.5" => Some(768),
        "all-MiniLM-L6-v2" => Some(384),
        "all-MiniLM-L12-v2" => Some(384),
        _ => None,
    }
}

/// Resolve a model name string to a fastembed `EmbeddingModel`.
fn resolve_model(name: &str) -> anyhow::Result<EmbeddingModel> {
    match name {
        "nomic-embed-text-v1.5" => Ok(EmbeddingModel::NomicEmbedTextV15),
        "all-MiniLM-L6-v2" => Ok(EmbeddingModel::AllMiniLML6V2),
        "all-MiniLM-L12-v2" => Ok(EmbeddingModel::AllMiniLML12V2),
        other => Err(anyhow::anyhow!(
            "Unknown embedding model '{}'. Supported: nomic-embed-text-v1.5, all-MiniLM-L6-v2, all-MiniLM-L12-v2",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = vec![1.0_f32, -0.5, 0.25, 0.0];
        let bytes = encode_vector(&original);
        let decoded = decode_vector(&bytes);
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1e-7, "roundtrip mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "identical vectors should have sim=1"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "orthogonal vectors should have sim=0");
    }

    #[test]
    fn test_cosine_zero_vector() {
        let a = vec![0.0_f32, 0.0, 0.0];
        let b = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
