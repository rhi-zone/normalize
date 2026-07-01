//! CFG dataflow: backward liveness solver + loaders for the `cfg_*` tables.
//!
//! normalize-facts owns the `cfg_blocks` / `cfg_edges` / `cfg_defs` / `cfg_uses`
//! tables (populated during index rebuild), so the DB-backed loaders and the pure
//! liveness fixed-point that consumes them live here rather than being duplicated
//! at each call site. Two consumers share this module: `normalize analyze liveness`
//! (the per-block report) and `normalize-refactor`'s extract-function recipe (which
//! derives parameters/return vars from live-in/live-out).

use std::collections::{BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// One row from `cfg_blocks`.
#[derive(Debug, Clone)]
pub struct CfgBlockRow {
    pub block_id: u32,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// One row from `cfg_edges`. Callers that only need control flow use `from`/`to`;
/// the extract-function recipe additionally consults `kind` / `exception_type`.
#[derive(Debug, Clone)]
pub struct CfgEdgeRow {
    pub from: u32,
    pub to: u32,
    pub kind: String,
    pub exception_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Loaders (query the cfg_* tables for one function)
// ---------------------------------------------------------------------------

/// Load all blocks for a function, ordered by `block_id`.
pub async fn load_blocks(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<CfgBlockRow>, String> {
    let mut rows = conn
        .query(
            "SELECT block_id, kind, start_line, end_line \
             FROM cfg_blocks \
             WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3 \
             ORDER BY block_id",
            libsql::params![
                file.to_string(),
                func_qname.to_string(),
                func_start_line as i64
            ],
        )
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
        let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
        let kind: String = row.get(1).map_err(|e| format!("DB error: {e}"))?;
        let start_line: i64 = row.get(2).map_err(|e| format!("DB error: {e}"))?;
        let end_line: i64 = row.get(3).map_err(|e| format!("DB error: {e}"))?;
        out.push(CfgBlockRow {
            block_id: block_id as u32,
            kind,
            start_line: start_line as u32,
            end_line: end_line as u32,
        });
    }
    Ok(out)
}

/// Load all edges for a function (with `kind` + `exception_type`).
pub async fn load_edges(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<CfgEdgeRow>, String> {
    let mut rows = conn
        .query(
            "SELECT from_block, to_block, kind, COALESCE(exception_type, '') \
             FROM cfg_edges \
             WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
            libsql::params![
                file.to_string(),
                func_qname.to_string(),
                func_start_line as i64
            ],
        )
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
        let from: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
        let to: i64 = row.get(1).map_err(|e| format!("DB error: {e}"))?;
        let kind: String = row.get(2).map_err(|e| format!("DB error: {e}"))?;
        let exc: String = row.get(3).map_err(|e| format!("DB error: {e}"))?;
        out.push(CfgEdgeRow {
            from: from as u32,
            to: to as u32,
            kind,
            exception_type: if exc.is_empty() { None } else { Some(exc) },
        });
    }
    Ok(out)
}

/// Load all `(block_id, name)` def rows for a function.
pub async fn load_defs(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<(u32, String)>, String> {
    load_block_names(conn, "cfg_defs", file, func_qname, func_start_line).await
}

/// Load all `(block_id, name)` use rows for a function.
pub async fn load_uses(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<(u32, String)>, String> {
    load_block_names(conn, "cfg_uses", file, func_qname, func_start_line).await
}

/// Shared `(block_id, name)` loader for the `cfg_defs` / `cfg_uses` tables.
async fn load_block_names(
    conn: &libsql::Connection,
    table: &str,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<(u32, String)>, String> {
    let sql = format!(
        "SELECT block_id, name \
         FROM {table} \
         WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3"
    );
    let mut rows = conn
        .query(
            &sql,
            libsql::params![
                file.to_string(),
                func_qname.to_string(),
                func_start_line as i64
            ],
        )
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
        let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
        let name: String = row.get(1).map_err(|e| format!("DB error: {e}"))?;
        out.push((block_id as u32, name));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Liveness fixed-point
// ---------------------------------------------------------------------------

/// Backward-dataflow liveness fixed-point over a function's CFG.
///
/// - `live_in[B]  = use[B] ∪ (live_out[B] − def[B])`
/// - `live_out[B] = ∪ { live_in[S] | S ∈ succ(B) }`
///
/// Blocks are processed in reverse `block_ids` order for faster convergence.
/// Returns `(live_in, live_out)` keyed by block id.
pub fn compute_liveness(
    block_ids: &[u32],
    defs: &HashMap<u32, BTreeSet<String>>,
    uses_map: &HashMap<u32, BTreeSet<String>>,
    succs: &HashMap<u32, Vec<u32>>,
    // normalize-syntax-allow: rust/tuple-return
) -> (
    HashMap<u32, BTreeSet<String>>,
    HashMap<u32, BTreeSet<String>>,
) {
    let mut live_in: HashMap<u32, BTreeSet<String>> = HashMap::new();
    let mut live_out: HashMap<u32, BTreeSet<String>> = HashMap::new();
    for id in block_ids {
        live_in.insert(*id, BTreeSet::new());
        live_out.insert(*id, BTreeSet::new());
    }

    let empty: BTreeSet<String> = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for &bid in block_ids.iter().rev() {
            let mut new_lo: BTreeSet<String> = BTreeSet::new();
            if let Some(succ_list) = succs.get(&bid) {
                for &s in succ_list {
                    if let Some(li) = live_in.get(&s) {
                        new_lo.extend(li.iter().cloned());
                    }
                }
            }

            let block_uses = uses_map.get(&bid).unwrap_or(&empty);
            let block_defs = defs.get(&bid).unwrap_or(&empty);
            let mut new_li: BTreeSet<String> = block_uses.clone();
            for v in &new_lo {
                if !block_defs.contains(v) {
                    new_li.insert(v.clone());
                }
            }

            if new_lo != *live_out.get(&bid).unwrap_or(&empty)
                || new_li != *live_in.get(&bid).unwrap_or(&empty)
            {
                changed = true;
                live_out.insert(bid, new_lo);
                live_in.insert(bid, new_li);
            }
        }
    }

    (live_in, live_out)
}
