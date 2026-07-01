//! Liveness analysis: live-in/live-out variable sets per CFG block.
//!
//! Queries the `cfg_blocks`, `cfg_defs`, and `cfg_uses` tables from the index,
//! runs the standard backward-dataflow liveness fixed-point, and returns a
//! report with per-block live-in and live-out sets.

use crate::output::OutputFormatter;
use normalize_facts::FileIndex;
use normalize_facts::cfg_dataflow;
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

    // Load CFG tables for this function (owned by normalize-facts).
    let block_rows = cfg_dataflow::load_blocks(conn, file, &func_name, func_start_line).await?;

    if block_rows.is_empty() {
        return Ok(LivenessReport {
            file: file.to_string(),
            function: func_name,
            function_start_line: func_start_line,
            blocks: vec![],
        });
    }

    let edge_rows = cfg_dataflow::load_edges(conn, file, &func_name, func_start_line).await?;
    let def_rows = cfg_dataflow::load_defs(conn, file, &func_name, func_start_line).await?;
    let use_rows = cfg_dataflow::load_uses(conn, file, &func_name, func_start_line).await?;

    // Build per-block defs and uses maps.
    let block_ids: Vec<u32> = block_rows.iter().map(|b| b.block_id).collect();
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
    for e in &edge_rows {
        succs.entry(e.from).or_default().push(e.to);
    }

    // Backward-dataflow liveness fixed-point.
    let (live_in, live_out) = cfg_dataflow::compute_liveness(&block_ids, &defs, &uses_map, &succs);

    // Build the report.
    let blocks: Vec<BlockLiveness> = block_rows
        .iter()
        .map(|b| {
            let mut li: Vec<String> = live_in
                .get(&b.block_id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            let mut lo: Vec<String> = live_out
                .get(&b.block_id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            li.sort();
            lo.sort();
            BlockLiveness {
                block_id: b.block_id,
                kind: b.kind.clone(),
                start_line: b.start_line,
                end_line: b.end_line,
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
