//! Effects analysis: side-effecting CFG constructs per function.
//!
//! Queries the `cfg_effects` table from the index and returns a summary of
//! suspension points, deferred calls, generator yields, and resource
//! acquisitions/releases within each function.

use crate::output::OutputFormatter;
use normalize_facts::FileIndex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// A single side-effect occurrence within a function.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EffectEntry {
    /// Kind of effect (e.g. "await", "defer", "yield", "acquire", "release", "send", "receive").
    pub kind: String,
    /// Block ID where the effect occurs.
    pub block_id: u32,
    /// Source line of the effect (1-indexed).
    pub line: u32,
    /// Optional label: resource name, expression text, etc.
    pub label: Option<String>,
}

/// Per-function effects summary.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FunctionEffects {
    /// Qualified function name.
    pub function: String,
    /// Function start line (1-indexed).
    pub function_start_line: u32,
    /// All effects in this function, ordered by line.
    pub effects: Vec<EffectEntry>,
}

/// Result of `normalize analyze effects`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EffectsReport {
    /// Source file path.
    pub file: String,
    /// Functions with at least one effect (or the filtered function).
    pub functions: Vec<FunctionEffects>,
}

impl OutputFormatter for EffectsReport {
    fn format_text(&self) -> String {
        if self.functions.is_empty() {
            return format!(
                "No effect data for {}.\nRun `normalize structure rebuild` first.",
                self.file
            );
        }
        let mut out = format!("Effects for {}\n", self.file);
        for func in &self.functions {
            // Count by kind
            let awaits: Vec<_> = func.effects.iter().filter(|e| e.kind == "await").collect();
            let defers: Vec<_> = func.effects.iter().filter(|e| e.kind == "defer").collect();
            let yields: Vec<_> = func.effects.iter().filter(|e| e.kind == "yield").collect();
            let acquires: Vec<_> = func
                .effects
                .iter()
                .filter(|e| e.kind == "acquire")
                .collect();
            let releases: Vec<_> = func
                .effects
                .iter()
                .filter(|e| e.kind == "release")
                .collect();
            let sends: Vec<_> = func.effects.iter().filter(|e| e.kind == "send").collect();
            let receives: Vec<_> = func
                .effects
                .iter()
                .filter(|e| e.kind == "receive")
                .collect();

            if func.effects.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\nFunction {} (line {}):\n",
                func.function, func.function_start_line
            ));

            if !awaits.is_empty() {
                let lines: Vec<_> = awaits.iter().map(|e| e.line.to_string()).collect();
                out.push_str(&format!(
                    "  Suspension points: {} (lines {})\n",
                    awaits.len(),
                    lines.join(", ")
                ));
            }
            if !defers.is_empty() {
                let lines: Vec<_> = defers.iter().map(|e| e.line.to_string()).collect();
                out.push_str(&format!(
                    "  Deferred calls: {} (lines {})\n",
                    defers.len(),
                    lines.join(", ")
                ));
            }
            if !yields.is_empty() {
                let lines: Vec<_> = yields.iter().map(|e| e.line.to_string()).collect();
                out.push_str(&format!(
                    "  Yields: {} (lines {})\n",
                    yields.len(),
                    lines.join(", ")
                ));
            }
            if !acquires.is_empty() {
                let labeled: Vec<_> = acquires
                    .iter()
                    .map(|e| {
                        if let Some(lbl) = &e.label {
                            format!("{} (line {})", lbl, e.line)
                        } else {
                            format!("line {}", e.line)
                        }
                    })
                    .collect();
                out.push_str(&format!(
                    "  Resource acquisitions: {}\n",
                    labeled.join(", ")
                ));
            }
            if !releases.is_empty() {
                let lines: Vec<_> = releases.iter().map(|e| e.line.to_string()).collect();
                out.push_str(&format!(
                    "  Resource releases: {} (lines {})\n",
                    releases.len(),
                    lines.join(", ")
                ));
            }
            if !sends.is_empty() {
                let lines: Vec<_> = sends.iter().map(|e| e.line.to_string()).collect();
                out.push_str(&format!(
                    "  Channel/goroutine sends: {} (lines {})\n",
                    sends.len(),
                    lines.join(", ")
                ));
            }
            if !receives.is_empty() {
                let lines: Vec<_> = receives.iter().map(|e| e.line.to_string()).collect();
                out.push_str(&format!(
                    "  Channel receives: {} (lines {})\n",
                    receives.len(),
                    lines.join(", ")
                ));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Query the index for CFG effects and return a per-function summary.
pub async fn analyze_effects(
    idx: &FileIndex,
    file: &str,
    function: Option<&str>,
) -> Result<EffectsReport, String> {
    let conn = idx.connection();

    // Query: optionally filtered by function name.
    let effect_rows: Vec<(String, u32, u32, String, u32, Option<String>)> = {
        let query = if function.is_some() {
            "SELECT function_qname, function_start_line, block_id, kind, line, label \
             FROM cfg_effects \
             WHERE file = ?1 AND function_qname = ?2 \
             ORDER BY function_start_line, line"
        } else {
            "SELECT function_qname, function_start_line, block_id, kind, line, label \
             FROM cfg_effects \
             WHERE file = ?1 \
             ORDER BY function_start_line, line"
        };

        let mut rows = if let Some(func) = function {
            conn.query(query, libsql::params![file.to_string(), func.to_string()])
                .await
                .map_err(|e| format!("DB error: {e}"))?
        } else {
            conn.query(query, libsql::params![file.to_string()])
                .await
                .map_err(|e| format!("DB error: {e}"))?
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {e}"))? {
            let func_name: String = row.get(0).map_err(|e| format!("DB error: {e}"))?;
            let func_start_line: i64 = row.get(1).map_err(|e| format!("DB error: {e}"))?;
            let block_id: i64 = row.get(2).map_err(|e| format!("DB error: {e}"))?;
            let kind: String = row.get(3).map_err(|e| format!("DB error: {e}"))?;
            let line: i64 = row.get(4).map_err(|e| format!("DB error: {e}"))?;
            let label: Option<String> = row.get(5).ok().flatten();
            out.push((
                func_name,
                func_start_line as u32,
                block_id as u32,
                kind,
                line as u32,
                label,
            ));
        }
        out
    };

    if effect_rows.is_empty() {
        return Ok(EffectsReport {
            file: file.to_string(),
            functions: vec![],
        });
    }

    // Group by function.
    let mut func_map: std::collections::BTreeMap<(String, u32), Vec<EffectEntry>> =
        std::collections::BTreeMap::new();

    for (func_name, func_sl, block_id, kind, line, label) in effect_rows {
        func_map
            .entry((func_name, func_sl))
            .or_default()
            .push(EffectEntry {
                kind,
                block_id,
                line,
                label: label.filter(|s| !s.is_empty()),
            });
    }

    let functions: Vec<FunctionEffects> = func_map
        .into_iter()
        .map(|((fname, fsl), effects)| FunctionEffects {
            function: fname,
            function_start_line: fsl,
            effects,
        })
        .collect();

    Ok(EffectsReport {
        file: file.to_string(),
        functions,
    })
}
