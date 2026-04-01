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
    DROP_EMBEDDINGS_IDX_MODEL, DROP_EMBEDDINGS_IDX_SOURCE, DROP_EMBEDDINGS_TABLE,
    DROP_VEC_EMBEDDINGS_TABLE, create_vec_embeddings_ddl,
};
use crate::search::StoredEmbedding;
use crate::vec_ext::VecConnection;
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
/// When a `VecConnection` is provided, the DDL is executed on it (where
/// sqlite-vec is registered).  Otherwise falls back to the main `libsql`
/// connection (which may not have vec, in which case the CREATE silently
/// fails).  Returns `true` if the table is available after this call.
pub async fn ensure_vec_schema(
    conn: &Connection,
    dims: usize,
    vec_conn: Option<&VecConnection>,
) -> bool {
    let ddl = create_vec_embeddings_ddl(dims);
    if let Some(vc) = vec_conn {
        vc.execute(&ddl).is_ok()
    } else {
        conn.execute(&ddl, ()).await.is_ok()
    }
}

/// Returns `true` if the `vec_embeddings` virtual table exists and is queryable.
///
/// When a `VecConnection` is provided, queries through it; otherwise falls
/// back to the `libsql` connection.
pub async fn vec_table_available(conn: &Connection, vec_conn: Option<&VecConnection>) -> bool {
    if let Some(vc) = vec_conn {
        vc.execute("SELECT rowid FROM vec_embeddings LIMIT 1")
            .is_ok()
    } else {
        conn.query("SELECT rowid FROM vec_embeddings LIMIT 1", ())
            .await
            .is_ok()
    }
}

/// Insert or replace one embedding row.
///
/// Uses `INSERT OR REPLACE` keyed on the UNIQUE constraint
/// `(source_type, source_path, source_id)` so re-indexing a symbol replaces the
/// old vector in a single statement — no SELECT-then-DELETE round-trip.
///
/// The corresponding row in `vec_embeddings` is also updated when a
/// `VecConnection` is provided (best-effort; errors are silently ignored).
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
    vec_conn: Option<&VecConnection>,
) -> Result<(), libsql::Error> {
    // INSERT OR REPLACE: if a row with the same (source_type, source_path, source_id)
    // already exists, SQLite deletes it and inserts the new one. This gives us a new
    // rowid, which we use for the vec_embeddings mirror.
    conn.execute(
        "INSERT OR REPLACE INTO embeddings (source_type, source_path, source_id, model, last_commit, staleness, chunk_text, embedding)
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
    // INSERT OR REPLACE handles the case where the old rowid was already present.
    let new_id: i64 = {
        let mut rows = conn.query("SELECT last_insert_rowid()", ()).await?;
        if let Some(row) = rows.next().await? {
            row.get(0)?
        } else {
            return Ok(());
        }
    };

    if let Some(vc) = vec_conn {
        if let Ok(stmt) =
            vc.prepare("INSERT OR REPLACE INTO vec_embeddings(rowid, embedding) VALUES (?1, ?2)")
        {
            stmt.bind_int64(1, new_id);
            stmt.bind_blob(2, embedding_bytes);
            let _ = stmt.step();
        }
    } else {
        let _ = conn
            .execute(
                "INSERT OR REPLACE INTO vec_embeddings(rowid, embedding) VALUES (?1, ?2)",
                params![new_id, embedding_bytes.to_vec()],
            )
            .await;
    }

    Ok(())
}

/// ANN search using the `vec_embeddings` virtual table.
///
/// Returns up to `k` candidate rows from `embeddings` ordered by vector
/// distance (closest first), ready for staleness re-ranking.
///
/// When a `VecConnection` is provided, uses it for the vec query (required for
/// sqlite-vec support).  Returns `None` if `vec_embeddings` is not available.
/// The caller should fall back to [`load_all_embeddings`] +
/// [`crate::search::rerank`] in that case.
pub async fn ann_search(
    conn: &Connection,
    model: &str,
    query_bytes: &[u8],
    k: usize,
    vec_conn: Option<&VecConnection>,
) -> Option<Vec<StoredEmbedding>> {
    // First verify the virtual table exists.
    if !vec_table_available(conn, vec_conn).await {
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

    if let Some(vc) = vec_conn {
        // Use the VecConnection for the ANN query (it has sqlite-vec registered).
        let stmt = vc.prepare(sql).ok()?;
        stmt.bind_blob(1, query_bytes);
        stmt.bind_int64(2, k as i64);
        stmt.bind_text(3, model);

        let mut result = Vec::new();
        while stmt.step().ok()? {
            let id = stmt.column_int64(0);
            let source_type = stmt.column_text(1).unwrap_or_default();
            let source_path = stmt.column_text(2).unwrap_or_default();
            let source_id_val = stmt.column_int64(3);
            let source_id = if source_id_val != 0 {
                Some(source_id_val)
            } else {
                None
            };
            let staleness = stmt.column_double(4) as f32;
            let chunk_text = stmt.column_text(5).unwrap_or_default();
            let last_commit = stmt.column_text(6);
            let blob = stmt.column_blob(7);
            let vector = crate::search::parse_blob(blob);

            result.push(StoredEmbedding {
                id,
                source_type,
                source_path,
                source_id,
                staleness,
                chunk_text,
                last_commit,
                vector,
            });
        }

        Some(result)
    } else {
        // Fallback: try through libsql connection (may not have vec loaded).
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
/// Also removes the corresponding rows from `vec_embeddings` (best-effort)
/// when a `VecConnection` is provided.
pub async fn delete_embeddings_for_path(
    conn: &Connection,
    source_path: &str,
    vec_conn: Option<&VecConnection>,
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
        if let Some(vc) = vec_conn {
            if let Ok(stmt) = vc.prepare("DELETE FROM vec_embeddings WHERE rowid = ?1") {
                stmt.bind_int64(1, id);
                let _ = stmt.step();
            }
        } else {
            let _ = conn
                .execute("DELETE FROM vec_embeddings WHERE rowid = ?1", params![id])
                .await;
        }
    }

    Ok(affected)
}

/// Drop embedding tables entirely for a full rebuild.
///
/// This is much faster than `DELETE FROM` for large tables — it avoids
/// generating tombstone pages that bloat the database file.  The caller
/// must call [`ensure_schema`] + [`ensure_vec_schema`] afterwards to
/// recreate the tables.
pub async fn drop_embedding_tables(
    conn: &Connection,
    vec_conn: Option<&VecConnection>,
) -> Result<(), libsql::Error> {
    if let Some(vc) = vec_conn {
        let _ = vc.execute(DROP_VEC_EMBEDDINGS_TABLE);
    } else {
        let _ = conn.execute(DROP_VEC_EMBEDDINGS_TABLE, ()).await;
    }
    conn.execute(DROP_EMBEDDINGS_IDX_SOURCE, ()).await?;
    conn.execute(DROP_EMBEDDINGS_IDX_MODEL, ()).await?;
    conn.execute(DROP_EMBEDDINGS_TABLE, ()).await?;
    Ok(())
}

/// Run `VACUUM` to reclaim space after a full rebuild.
pub async fn vacuum(conn: &Connection) {
    let _ = conn.execute("VACUUM", ()).await;
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
