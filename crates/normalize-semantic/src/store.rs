//! Embedding storage: read/write embeddings from/to the structural index SQLite.
//!
//! Operates through direct `libsql` calls on the same connection as `FileIndex`.
//! The embeddings table is created lazily on first write.

use crate::schema::{
    CREATE_EMBEDDINGS_IDX_MODEL, CREATE_EMBEDDINGS_IDX_SOURCE, CREATE_EMBEDDINGS_TABLE,
};
use crate::search::StoredEmbedding;
use libsql::{Connection, params};

/// Ensure the embeddings table and indices exist.
pub async fn ensure_schema(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute(CREATE_EMBEDDINGS_TABLE, ()).await?;
    conn.execute(CREATE_EMBEDDINGS_IDX_SOURCE, ()).await?;
    conn.execute(CREATE_EMBEDDINGS_IDX_MODEL, ()).await?;
    Ok(())
}

/// Insert or replace one embedding row.
///
/// Upserts based on `(source_type, source_path, source_id)` so re-indexing a
/// symbol replaces the old vector rather than appending.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_embedding(
    conn: &Connection,
    source_type: &str,
    source_path: &str,
    source_id: Option<i64>,
    model: &str,
    last_commit: Option<&str>,
    staleness: f32,
    chunk_text: &str,
    embedding_bytes: &[u8],
) -> Result<(), libsql::Error> {
    // Delete any existing row for this (source_type, source_path, source_id) regardless of model.
    // This handles model changes cleanly (old vectors for a different model are cleared).
    if let Some(sid) = source_id {
        conn.execute(
            "DELETE FROM embeddings WHERE source_type = ?1 AND source_path = ?2 AND source_id = ?3",
            params![source_type, source_path, sid],
        )
        .await?;
    } else {
        conn.execute(
            "DELETE FROM embeddings WHERE source_type = ?1 AND source_path = ?2 AND source_id IS NULL",
            params![source_type, source_path],
        )
        .await?;
    }

    conn.execute(
        "INSERT INTO embeddings (source_type, source_path, source_id, model, last_commit, staleness, chunk_text, embedding)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            source_type,
            source_path,
            source_id,
            model,
            last_commit,
            staleness as f64,
            chunk_text,
            embedding_bytes.to_vec()
        ],
    )
    .await?;

    Ok(())
}

/// Load all stored embeddings for a given model name.
///
/// Returns all rows so the caller can do brute-force cosine search in memory.
/// For larger repos this can be replaced with a sqlite-vec virtual table query.
pub async fn load_all_embeddings(
    conn: &Connection,
    model: &str,
) -> Result<Vec<StoredEmbedding>, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT id, source_type, source_path, source_id, staleness, chunk_text, last_commit, embedding
             FROM embeddings WHERE model = ?1",
            params![model],
        )
        .await?;

    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: i64 = row.get(0)?;
        let source_type: String = row.get(1)?;
        let source_path: String = row.get(2)?;
        let source_id: Option<i64> = row.get(3)?;
        let staleness: f64 = row.get(4)?;
        let chunk_text: String = row.get(5)?;
        let last_commit: Option<String> = row.get(6)?;
        let blob: Vec<u8> = row.get(7)?;

        let vector = crate::search::parse_blob(blob);

        result.push(StoredEmbedding {
            id,
            source_type,
            source_path,
            source_id,
            staleness: staleness as f32,
            chunk_text,
            last_commit,
            vector,
        });
    }

    Ok(result)
}

/// Count embeddings stored for a given model.
pub async fn count_embeddings(conn: &Connection, model: &str) -> Result<i64, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM embeddings WHERE model = ?1",
            params![model],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get(0)?)
    } else {
        Ok(0)
    }
}

/// Delete embeddings for a specific (source_type, source_path) pair, all models.
/// Used during incremental rebuild when a file changes.
pub async fn delete_embeddings_for_path(
    conn: &Connection,
    source_path: &str,
) -> Result<u64, libsql::Error> {
    conn.execute(
        "DELETE FROM embeddings WHERE source_path = ?1",
        params![source_path],
    )
    .await
}

/// Delete all embeddings (full rebuild).
pub async fn clear_all_embeddings(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute("DELETE FROM embeddings", ()).await?;
    Ok(())
}

/// Return the set of file paths that have at least one embedding for the given model.
pub async fn embedded_paths(
    conn: &Connection,
    model: &str,
) -> Result<std::collections::HashSet<String>, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT DISTINCT source_path FROM embeddings WHERE model = ?1",
            params![model],
        )
        .await?;
    let mut set = std::collections::HashSet::new();
    while let Some(row) = rows.next().await? {
        set.insert(row.get::<String>(0)?);
    }
    Ok(set)
}
