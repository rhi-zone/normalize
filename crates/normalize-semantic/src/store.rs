//! Embedding storage: read/write embeddings from/to the structural index SQLite.
//!
//! Operates through direct `libsql` calls on the same connection as `FileIndex`.
//! The embeddings table is created lazily on first write.
//!
//! When sqlite-vec is available (`vec_embeddings` virtual table exists), inserts
//! and deletes are mirrored there so that [`ann_search`] can use the ANN index
//! instead of loading all vectors into memory.

use crate::schema::{
    CREATE_EMBEDDINGS_IDX_MODEL, CREATE_EMBEDDINGS_IDX_SOURCE, CREATE_EMBEDDINGS_TABLE,
    create_vec_embeddings_ddl,
};
use crate::search::StoredEmbedding;
use libsql::{Connection, params};

/// Number of ANN candidates to retrieve before staleness re-ranking.
///
/// Fetching more candidates gives the re-ranker more material to work with;
/// the caller can truncate to a smaller `top_k` afterwards.
pub const ANN_CANDIDATE_COUNT: usize = 50;

/// Ensure the embeddings table and indices exist.
pub async fn ensure_schema(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute(CREATE_EMBEDDINGS_TABLE, ()).await?;
    conn.execute(CREATE_EMBEDDINGS_IDX_SOURCE, ()).await?;
    conn.execute(CREATE_EMBEDDINGS_IDX_MODEL, ()).await?;
    Ok(())
}

/// Ensure the `vec_embeddings` ANN virtual table exists.
///
/// This is a no-op if sqlite-vec is not loaded (the CREATE VIRTUAL TABLE
/// statement will fail, which we swallow).  Returns `true` if the table is
/// available after this call.
pub async fn ensure_vec_schema(conn: &Connection, dims: usize) -> bool {
    let ddl = create_vec_embeddings_ddl(dims);
    conn.execute(&ddl, ()).await.is_ok()
}

/// Returns `true` if the `vec_embeddings` virtual table exists and is queryable.
pub async fn vec_table_available(conn: &Connection) -> bool {
    conn.query("SELECT rowid FROM vec_embeddings LIMIT 1", ())
        .await
        .is_ok()
}

/// Insert or replace one embedding row.
///
/// Upserts based on `(source_type, source_path, source_id)` so re-indexing a
/// symbol replaces the old vector rather than appending.
///
/// The corresponding row in `vec_embeddings` is also updated when the virtual
/// table is available (best-effort; errors are silently ignored).
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
    // Collect the IDs of rows we are about to replace so we can remove them
    // from vec_embeddings as well.
    let deleted_ids: Vec<i64> = if let Some(sid) = source_id {
        let mut rows = conn.query(
            "SELECT id FROM embeddings WHERE source_type = ?1 AND source_path = ?2 AND source_id = ?3",
            params![source_type, source_path, sid],
        ).await?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await? {
            ids.push(row.get::<i64>(0)?);
        }
        conn.execute(
            "DELETE FROM embeddings WHERE source_type = ?1 AND source_path = ?2 AND source_id = ?3",
            params![source_type, source_path, sid],
        )
        .await?;
        ids
    } else {
        let mut rows = conn.query(
            "SELECT id FROM embeddings WHERE source_type = ?1 AND source_path = ?2 AND source_id IS NULL",
            params![source_type, source_path],
        ).await?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await? {
            ids.push(row.get::<i64>(0)?);
        }
        conn.execute(
            "DELETE FROM embeddings WHERE source_type = ?1 AND source_path = ?2 AND source_id IS NULL",
            params![source_type, source_path],
        )
        .await?;
        ids
    };

    // Remove the old rows from the ANN index (best-effort; ignore errors if table absent).
    for old_id in &deleted_ids {
        let _ = conn
            .execute(
                "DELETE FROM vec_embeddings WHERE rowid = ?1",
                params![*old_id],
            )
            .await;
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

    // Mirror into vec_embeddings for ANN queries (best-effort).
    let new_id: i64 = {
        let mut rows = conn.query("SELECT last_insert_rowid()", ()).await?;
        if let Some(row) = rows.next().await? {
            row.get(0)?
        } else {
            return Ok(());
        }
    };

    let _ = conn
        .execute(
            "INSERT INTO vec_embeddings(rowid, embedding) VALUES (?1, ?2)",
            params![new_id, embedding_bytes.to_vec()],
        )
        .await;

    Ok(())
}

/// ANN search using the `vec_embeddings` virtual table.
///
/// Returns up to `k` candidate rows from `embeddings` ordered by vector
/// distance (closest first), ready for staleness re-ranking.
///
/// Returns `None` if `vec_embeddings` is not available (sqlite-vec not loaded
/// or table doesn't exist yet).  The caller should fall back to
/// [`load_all_embeddings`] + [`crate::search::rerank`] in that case.
pub async fn ann_search(
    conn: &Connection,
    model: &str,
    query_bytes: &[u8],
    k: usize,
) -> Option<Vec<StoredEmbedding>> {
    // First verify the virtual table exists.
    if !vec_table_available(conn).await {
        return None;
    }

    // sqlite-vec ANN query: returns rowid + distance for the k nearest vectors.
    // We JOIN back to `embeddings` to get all metadata in one round-trip.
    //
    // Note: `v.k` is a hidden column consumed by the vec0 module as the
    // result-limit parameter; `v.embedding MATCH ?1` supplies the query vector.
    let sql = "
        SELECT e.id, e.source_type, e.source_path, e.source_id,
               e.staleness, e.chunk_text, e.last_commit, e.embedding
        FROM vec_embeddings v
        JOIN embeddings e ON e.id = v.rowid
        WHERE v.embedding MATCH ?1
          AND v.k = ?2
          AND e.model = ?3
        ORDER BY v.distance
    ";

    let mut rows = conn
        .query(sql, params![query_bytes.to_vec(), k as i64, model])
        .await
        .ok()?;

    let mut result = Vec::new();
    while let Some(row) = rows.next().await.ok()? {
        let id: i64 = row.get(0).ok()?;
        let source_type: String = row.get(1).ok()?;
        let source_path: String = row.get(2).ok()?;
        let source_id: Option<i64> = row.get(3).ok()?;
        let staleness: f64 = row.get(4).ok()?;
        let chunk_text: String = row.get(5).ok()?;
        let last_commit: Option<String> = row.get(6).ok()?;
        let blob: Vec<u8> = row.get(7).ok()?;

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

    Some(result)
}

/// Load all stored embeddings for a given model name.
///
/// Returns all rows so the caller can do brute-force cosine search in memory.
/// Prefer [`ann_search`] when the sqlite-vec virtual table is available.
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
///
/// Also removes the corresponding rows from `vec_embeddings` (best-effort).
pub async fn delete_embeddings_for_path(
    conn: &Connection,
    source_path: &str,
) -> Result<u64, libsql::Error> {
    // Collect IDs before deletion so we can clean up vec_embeddings.
    let mut rows = conn
        .query(
            "SELECT id FROM embeddings WHERE source_path = ?1",
            params![source_path],
        )
        .await?;
    let mut ids: Vec<i64> = Vec::new();
    while let Some(row) = rows.next().await? {
        ids.push(row.get(0)?);
    }

    let affected = conn
        .execute(
            "DELETE FROM embeddings WHERE source_path = ?1",
            params![source_path],
        )
        .await?;

    for id in ids {
        let _ = conn
            .execute("DELETE FROM vec_embeddings WHERE rowid = ?1", params![id])
            .await;
    }

    Ok(affected)
}

/// Delete all embeddings (full rebuild).
///
/// Also truncates `vec_embeddings` (best-effort).
pub async fn clear_all_embeddings(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute("DELETE FROM embeddings", ()).await?;
    let _ = conn.execute("DELETE FROM vec_embeddings", ()).await;
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
