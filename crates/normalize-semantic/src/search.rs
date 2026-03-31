//! ANN search with staleness-based re-ranking.
//!
//! Search flow:
//! 1. Embed the query string with the same model used at index time.
//! 2. Load all stored embeddings (for the active model) from SQLite.
//! 3. Compute cosine similarity between query and each stored vector.
//! 4. Re-rank: `score = cosine_sim * (1 - staleness_weight * staleness)`.
//! 5. Return top-K results, sorted by final score descending.
//!
//! For small-to-medium repos this brute-force ANN is fast enough.
//! The sqlite-vec extension can be wired up for larger repos when needed.

use crate::embedder::{cosine_similarity, decode_vector};

/// Weight applied to staleness during re-ranking. Tunable.
const STALENESS_WEIGHT: f32 = 0.3;

/// One result from a semantic search.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Row id in the embeddings table.
    pub id: i64,
    /// Source type tag ("symbol", "doc", …).
    pub source_type: String,
    /// Relative file path.
    pub source_path: String,
    /// FK into symbols table (if a symbol chunk).
    pub source_id: Option<i64>,
    /// Cosine similarity before re-ranking.
    pub similarity: f32,
    /// Staleness score stored at index time.
    pub staleness: f32,
    /// Final score after staleness penalty.
    pub score: f32,
    /// The chunk text that was embedded.
    pub chunk_text: String,
    /// Git commit SHA when this chunk was last embedded.
    pub last_commit: Option<String>,
}

/// In-memory representation of a stored embedding row (for brute-force search).
pub struct StoredEmbedding {
    pub id: i64,
    pub source_type: String,
    pub source_path: String,
    pub source_id: Option<i64>,
    pub staleness: f32,
    pub chunk_text: String,
    pub last_commit: Option<String>,
    pub vector: Vec<f32>,
}

/// Re-rank a list of stored embeddings against a query vector.
///
/// Returns hits sorted by final score descending, limited to `top_k`.
pub fn rerank(query_vec: &[f32], stored: Vec<StoredEmbedding>, top_k: usize) -> Vec<SearchHit> {
    let mut hits: Vec<SearchHit> = stored
        .into_iter()
        .map(|e| {
            let similarity = cosine_similarity(query_vec, &e.vector);
            let score = similarity * (1.0 - STALENESS_WEIGHT * e.staleness);
            SearchHit {
                id: e.id,
                source_type: e.source_type,
                source_path: e.source_path,
                source_id: e.source_id,
                similarity,
                staleness: e.staleness,
                score,
                chunk_text: e.chunk_text,
                last_commit: e.last_commit,
            }
        })
        .collect();

    // Sort descending by final score
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(top_k);
    hits
}

/// Parse a raw BLOB from the database into a f32 vector via `decode_vector`.
pub fn parse_blob(blob: Vec<u8>) -> Vec<f32> {
    decode_vector(&blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stored(id: i64, vec: Vec<f32>, staleness: f32) -> StoredEmbedding {
        StoredEmbedding {
            id,
            source_type: "symbol".to_string(),
            source_path: "src/lib.rs".to_string(),
            source_id: Some(id),
            staleness,
            chunk_text: "test chunk".to_string(),
            last_commit: None,
            vector: vec,
        }
    }

    #[test]
    fn test_rerank_orders_by_score() {
        let query = vec![1.0_f32, 0.0, 0.0];
        let stored = vec![
            make_stored(1, vec![1.0, 0.0, 0.0], 0.0), // sim=1.0, staleness=0 → score=1.0
            make_stored(2, vec![0.0, 1.0, 0.0], 0.0), // sim=0.0 → score=0.0
            make_stored(3, vec![0.9, 0.4, 0.0], 0.5), // lower final score due to staleness
        ];
        let hits = rerank(&query, stored, 3);
        assert_eq!(hits[0].id, 1, "most similar, no staleness should be first");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn test_rerank_respects_top_k() {
        let query = vec![1.0_f32, 0.0];
        let stored = (0..10)
            .map(|i| make_stored(i, vec![1.0, 0.0], 0.0))
            .collect();
        let hits = rerank(&query, stored, 3);
        assert_eq!(hits.len(), 3);
    }
}
