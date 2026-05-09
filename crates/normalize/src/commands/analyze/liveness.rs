//! Liveness analysis: live-in/live-out variable sets per CFG block.
//!
//! Queries the `cfg_blocks`, `cfg_defs`, and `cfg_uses` tables from the index,
//! runs the standard backward-dataflow liveness fixed-point, and returns a
//! report with per-block live-in and live-out sets.

use crate::output::OutputFormatter;
use normalize_facts::FileIndex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Live-in / live-out variable sets for one basic block.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BlockLiveness {
    /// Block ID (0-based, unique within the function).
    pub block_id: u32,
    /// Structural kind of this block (e.g. "entry", "exit", "branch").
    pub kind: String,
    /// First source line covered by this block (1-indexed).
    pub start_line: u32,
    /// Last source line covered by this block (1-indexed).
    pub end_line: u32,
    /// Variables live at the entry of this block.
    pub live_in: Vec<String>,
    /// Variables live at the exit of this block.
    pub live_out: Vec<String>,
}

/// Result of `normalize analyze liveness`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LivenessReport {
    /// Source file path.
    pub file: String,
    /// Function name (or filter used).
    pub function: String,
    /// Function start line (1-indexed).
    pub function_start_line: u32,
    /// Per-block liveness sets, ordered by block_id.
    pub blocks: Vec<BlockLiveness>,
}

impl OutputFormatter for LivenessReport {
    fn format_text(&self) -> String {
        if self.blocks.is_empty() {
            return format!(
                "No CFG data for function '{}' in {}.\n\
                 Run `normalize structure rebuild` first.",
                self.function, self.file
            );
        }
        let mut out = format!(
            "Liveness for {} (line {}) in {}\n",
            self.function, self.function_start_line, self.file
        );
        for blk in &self.blocks {
            let live_in_str = if blk.live_in.is_empty() {
                "∅".to_string()
            } else {
                blk.live_in.join(", ")
            };
            let live_out_str = if blk.live_out.is_empty() {
                "∅".to_string()
            } else {
                blk.live_out.join(", ")
            };
            out.push_str(&format!(
                "\nBlock {} ({}, line {}-{}):\n  live-in:  {}\n  live-out: {}\n",
                blk.block_id, blk.kind, blk.start_line, blk.end_line, live_in_str, live_out_str,
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Run liveness analysis for a function in a file.
///
/// Queries the index for CFG data, then runs backward-dataflow fixed-point
/// to compute live-in and live-out sets.
pub async fn analyze_liveness(
    idx: &FileIndex,
    file: &str,
    function: &str,
) -> Result<LivenessReport, String> {
    let conn = idx.connection();

    // Find matching functions (file + function name; pick first if multiple).
    let func_row = {
        let mut rows = conn
            .query(
                "SELECT DISTINCT function_qname, function_start_line \
                 FROM cfg_blocks \
                 WHERE file = ?1 AND function_qname = ?2 \
                 ORDER BY function_start_line \
                 LIMIT 1",
                libsql::params![file.to_string(), function.to_string()],
            )
            .await
            .map_err(|e| format!("DB error: {e}"))?;
        match rows.next().await.map_err(|e| format!("DB error: {e}"))? {
            Some(row) => {
                let qname: String = row.get(0).map_err(|e| format!("DB error: {e}"))?;
                let fsl: i64 = row.get(1).map_err(|e| format!("DB error: {e}"))?;
                Some((qname, fsl as u32))
            }
            None => None,
        }
    };

    let (func_name, func_start_line) = match func_row {
        Some(r) => r,
        None => {
            return Ok(LivenessReport {
                file: file.to_string(),
                function: function.to_string(),
                function_start_line: 0,
                blocks: vec![],
            });
        }
    };

    // Load blocks.
    let block_rows: Vec<(u32, String, u32, u32)> = {
        let mut rows = conn
            .query(
                "SELECT block_id, kind, start_line, end_line \
                 FROM cfg_blocks \
                 WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3 \
                 ORDER BY block_id",
                libsql::params![file.to_string(), func_name.clone(), func_start_line as i64],
            )
            .await
            .map_err(|e| format!("DB error: {e}"))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
            let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
            let kind: String = row.get(1).map_err(|e| format!("DB error: {e}"))?;
            let start_line: i64 = row.get(2).map_err(|e| format!("DB error: {e}"))?;
            let end_line: i64 = row.get(3).map_err(|e| format!("DB error: {e}"))?;
            out.push((block_id as u32, kind, start_line as u32, end_line as u32));
        }
        out
    };

    if block_rows.is_empty() {
        return Ok(LivenessReport {
            file: file.to_string(),
            function: func_name,
            function_start_line: func_start_line,
            blocks: vec![],
        });
    }

    // Load edges (from_block -> to_block).
    let edge_rows: Vec<(u32, u32)> = {
        let mut rows = conn
            .query(
                "SELECT from_block, to_block \
                 FROM cfg_edges \
                 WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
                libsql::params![file.to_string(), func_name.clone(), func_start_line as i64],
            )
            .await
            .map_err(|e| format!("DB error: {e}"))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
            let from: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
            let to: i64 = row.get(1).map_err(|e| format!("DB error: {e}"))?;
            out.push((from as u32, to as u32));
        }
        out
    };

    // Load defs (block_id -> set of names).
    let def_rows: Vec<(u32, String)> = {
        let mut rows = conn
            .query(
                "SELECT block_id, name \
                 FROM cfg_defs \
                 WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
                libsql::params![file.to_string(), func_name.clone(), func_start_line as i64],
            )
            .await
            .map_err(|e| format!("DB error: {e}"))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
            let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
            let name: String = row.get(1).map_err(|e| format!("DB error: {e}"))?;
            out.push((block_id as u32, name));
        }
        out
    };

    // Load uses (block_id -> set of names).
    let use_rows: Vec<(u32, String)> = {
        let mut rows = conn
            .query(
                "SELECT block_id, name \
                 FROM cfg_uses \
                 WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
                libsql::params![file.to_string(), func_name.clone(), func_start_line as i64],
            )
            .await
            .map_err(|e| format!("DB error: {e}"))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
            let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {e}"))?;
            let name: String = row.get(1).map_err(|e| format!("DB error: {e}"))?;
            out.push((block_id as u32, name));
        }
        out
    };

    // Build per-block defs and uses maps.
    let block_ids: Vec<u32> = block_rows.iter().map(|(id, _, _, _)| *id).collect();
    let mut defs: std::collections::HashMap<u32, std::collections::BTreeSet<String>> =
        std::collections::HashMap::new();
    let mut uses_map: std::collections::HashMap<u32, std::collections::BTreeSet<String>> =
        std::collections::HashMap::new();
    for id in &block_ids {
        defs.insert(*id, std::collections::BTreeSet::new());
        uses_map.insert(*id, std::collections::BTreeSet::new());
    }
    for (bid, name) in def_rows {
        defs.entry(bid).or_default().insert(name);
    }
    for (bid, name) in use_rows {
        uses_map.entry(bid).or_default().insert(name);
    }

    // Build successor map (block -> successors).
    let mut succs: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for id in &block_ids {
        succs.insert(*id, Vec::new());
    }
    for (from, to) in &edge_rows {
        succs.entry(*from).or_default().push(*to);
    }

    // Backward dataflow: iterate until fixed point.
    // live_in[B]  = use[B] ∪ (live_out[B] − def[B])
    // live_out[B] = ∪ { live_in[S] | S ∈ succ(B) }
    let mut live_in: std::collections::HashMap<u32, std::collections::BTreeSet<String>> =
        std::collections::HashMap::new();
    let mut live_out: std::collections::HashMap<u32, std::collections::BTreeSet<String>> =
        std::collections::HashMap::new();
    for id in &block_ids {
        live_in.insert(*id, std::collections::BTreeSet::new());
        live_out.insert(*id, std::collections::BTreeSet::new());
    }

    let mut changed = true;
    while changed {
        changed = false;
        // Process blocks in reverse order for faster convergence.
        for &bid in block_ids.iter().rev() {
            // live_out[B] = ∪ { live_in[S] }
            let mut new_live_out: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            if let Some(succ_list) = succs.get(&bid) {
                for &s in succ_list {
                    if let Some(li) = live_in.get(&s) {
                        new_live_out.extend(li.iter().cloned());
                    }
                }
            }

            // live_in[B] = use[B] ∪ (live_out[B] − def[B])
            let empty_uses: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let empty_defs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let block_uses = uses_map.get(&bid).unwrap_or(&empty_uses);
            let block_defs = defs.get(&bid).unwrap_or(&empty_defs);
            let mut new_live_in: std::collections::BTreeSet<String> = block_uses.clone();
            for v in &new_live_out {
                if !block_defs.contains(v) {
                    new_live_in.insert(v.clone());
                }
            }

            let old_lo = live_out.get(&bid).cloned().unwrap_or_default();
            let old_li = live_in.get(&bid).cloned().unwrap_or_default();
            if new_live_out != old_lo || new_live_in != old_li {
                changed = true;
                live_out.insert(bid, new_live_out);
                live_in.insert(bid, new_live_in);
            }
        }
    }

    // Build the report.
    let blocks: Vec<BlockLiveness> = block_rows
        .iter()
        .map(|(bid, kind, sl, el)| {
            let mut li: Vec<String> = live_in
                .get(bid)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            let mut lo: Vec<String> = live_out
                .get(bid)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            li.sort();
            lo.sort();
            BlockLiveness {
                block_id: *bid,
                kind: kind.clone(),
                start_line: *sl,
                end_line: *el,
                live_in: li,
                live_out: lo,
            }
        })
        .collect();

    Ok(LivenessReport {
        file: file.to_string(),
        function: func_name,
        function_start_line: func_start_line,
        blocks,
    })
}
