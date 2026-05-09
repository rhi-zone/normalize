//! Exception flow analysis: type-refined throw→catch mappings per function.
//!
//! Queries the `cfg_edges` and `cfg_blocks` tables from the index and returns a
//! per-function summary of thrown exceptions, their catch clauses, and any
//! unhandled throws that escape the function.

use crate::output::OutputFormatter;
use normalize_facts::FileIndex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// A single throw site within a function.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThrowEntry {
    /// Block ID of the throw.
    pub block_id: u32,
    /// Source line of the throw block (1-indexed).
    pub line: u32,
    /// Exception type thrown, if known (None = unknown/conservative).
    pub exception_type: Option<String>,
    /// Block IDs of catch clauses that handle this throw (empty = unhandled).
    pub caught_by: Vec<u32>,
    /// Source lines of the catch blocks that handle this throw.
    pub caught_at_lines: Vec<u32>,
}

/// A single catch clause within a function.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CatchEntry {
    /// Block ID of the catch block.
    pub block_id: u32,
    /// Source line of the catch block (1-indexed).
    pub line: u32,
    /// Exception type(s) handled by this catch clause (empty = catches all).
    pub exception_types: Vec<String>,
    /// Number of throws that route to this catch clause.
    pub handled_count: u32,
}

/// Per-function exception flow summary.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FunctionExceptions {
    /// Qualified function name.
    pub function: String,
    /// Function start line (1-indexed).
    pub function_start_line: u32,
    /// All throw sites in this function.
    pub throws: Vec<ThrowEntry>,
    /// All catch clauses in this function.
    pub catches: Vec<CatchEntry>,
}

/// Result of `normalize analyze exceptions`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExceptionsReport {
    /// Source file path.
    pub file: String,
    /// Functions with at least one throw or catch (or the filtered function).
    pub functions: Vec<FunctionExceptions>,
}

impl OutputFormatter for ExceptionsReport {
    fn format_text(&self) -> String {
        if self.functions.is_empty() {
            return format!(
                "No exception data for {}.\nRun `normalize structure rebuild` first.",
                self.file
            );
        }
        let mut out = format!("Exceptions for {}\n", self.file);
        for func in &self.functions {
            if func.throws.is_empty() && func.catches.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\nFunction {} (line {}):\n",
                func.function, func.function_start_line
            ));

            if !func.throws.is_empty() {
                out.push_str("  Throws:\n");
                for throw in &func.throws {
                    let type_str = throw.exception_type.as_deref().unwrap_or("(unknown)");
                    if throw.caught_by.is_empty() {
                        out.push_str(&format!(
                            "    line {}: {} → unhandled (escapes function)\n",
                            throw.line, type_str
                        ));
                    } else {
                        let catch_lines: Vec<String> = throw
                            .caught_at_lines
                            .iter()
                            .map(|l| l.to_string())
                            .collect();
                        out.push_str(&format!(
                            "    line {}: {} → caught at line{} {}\n",
                            throw.line,
                            type_str,
                            if catch_lines.len() == 1 { "" } else { "s" },
                            catch_lines.join(", ")
                        ));
                    }
                }
            }

            if !func.catches.is_empty() {
                out.push_str("  Catch clauses:\n");
                for catch in &func.catches {
                    let types_str = if catch.exception_types.is_empty() {
                        "*".to_string()
                    } else {
                        catch.exception_types.join(" | ")
                    };
                    let handled = catch.handled_count;
                    if handled == 0 {
                        out.push_str(&format!(
                            "    line {}: {} — handles 0 throws (dead catch?)\n",
                            catch.line, types_str
                        ));
                    } else {
                        out.push_str(&format!(
                            "    line {}: {} — handles {} throw{}\n",
                            catch.line,
                            types_str,
                            handled,
                            if handled == 1 { "" } else { "s" }
                        ));
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Query the index for CFG exception edges and return a per-function summary.
pub async fn analyze_exceptions(
    idx: &FileIndex,
    file: &str,
    function: Option<&str>,
) -> Result<ExceptionsReport, String> {
    let conn = idx.connection();

    // Load all exception edges for the file (optionally filtered by function).
    // We query: from_block, to_block, exception_type (nullable), function_qname, function_start_line
    let edge_rows: Vec<(String, u32, u32, u32, Option<String>)> = {
        let query = if function.is_some() {
            "SELECT function_qname, function_start_line, from_block, to_block, exception_type \
             FROM cfg_edges \
             WHERE file = ?1 AND function_qname = ?2 AND kind = 'exception' \
             ORDER BY function_start_line, from_block"
        } else {
            "SELECT function_qname, function_start_line, from_block, to_block, exception_type \
             FROM cfg_edges \
             WHERE file = ?1 AND kind = 'exception' \
             ORDER BY function_start_line, from_block"
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
            let from_block: i64 = row.get(2).map_err(|e| format!("DB error: {e}"))?;
            let to_block: i64 = row.get(3).map_err(|e| format!("DB error: {e}"))?;
            let exception_type: Option<String> = row.get(4).ok().flatten();
            out.push((
                func_name,
                func_start_line as u32,
                from_block as u32,
                to_block as u32,
                exception_type,
            ));
        }
        out
    };

    if edge_rows.is_empty() {
        return Ok(ExceptionsReport {
            file: file.to_string(),
            functions: vec![],
        });
    }

    // Load all blocks for the file to get block kinds and start lines.
    let block_rows: Vec<(String, u32, u32, String, u32)> = {
        let query = if function.is_some() {
            "SELECT function_qname, function_start_line, block_id, kind, start_line \
             FROM cfg_blocks \
             WHERE file = ?1 AND function_qname = ?2 \
             ORDER BY function_start_line, block_id"
        } else {
            "SELECT function_qname, function_start_line, block_id, kind, start_line \
             FROM cfg_blocks \
             WHERE file = ?1 \
             ORDER BY function_start_line, block_id"
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
            let start_line: i64 = row.get(4).map_err(|e| format!("DB error: {e}"))?;
            out.push((
                func_name,
                func_start_line as u32,
                block_id as u32,
                kind,
                start_line as u32,
            ));
        }
        out
    };

    // Build block lookup: (func_name, func_start_line, block_id) → (kind, start_line)
    let mut block_map: std::collections::HashMap<(String, u32, u32), (String, u32)> =
        std::collections::HashMap::new();
    for (func_name, fsl, block_id, kind, start_line) in &block_rows {
        block_map.insert(
            (func_name.clone(), *fsl, *block_id),
            (kind.clone(), *start_line),
        );
    }

    // Collect all functions that appear in the edge rows.
    let mut func_keys: Vec<(String, u32)> = edge_rows
        .iter()
        .map(|(fn_, fsl, _, _, _)| (fn_.clone(), *fsl))
        .collect();
    func_keys.sort();
    func_keys.dedup();

    let mut functions: Vec<FunctionExceptions> = Vec::new();

    for (func_name, fsl) in func_keys {
        // All exception edges for this function.
        let func_edges: Vec<_> = edge_rows
            .iter()
            .filter(|(fn_, fs, _, _, _)| fn_ == &func_name && *fs == fsl)
            .collect();

        // Identify exit block id for this function.
        let exit_block_id = block_map
            .iter()
            .filter(|((fn_, fs, _), (kind, _))| fn_ == &func_name && *fs == fsl && kind == "exit")
            .map(|((_, _, bid), _)| *bid)
            .next();

        // Group exception edges: from_block → Vec<(to_block, exception_type)>
        let mut from_edges: std::collections::HashMap<u32, Vec<(u32, Option<String>)>> =
            std::collections::HashMap::new();
        for (_, _, from, to, exc_type) in func_edges {
            from_edges
                .entry(*from)
                .or_default()
                .push((*to, exc_type.clone()));
        }

        // Identify catch blocks: to_blocks that are BlockKind::Catch.
        let mut catch_block_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for edges in from_edges.values() {
            for (to, _) in edges {
                if block_map
                    .get(&(func_name.clone(), fsl, *to))
                    .is_some_and(|(kind, _)| kind == "catch")
                {
                    catch_block_ids.insert(*to);
                }
            }
        }

        // Build throw entries.
        let mut throws: Vec<ThrowEntry> = Vec::new();
        let mut sorted_froms: Vec<u32> = from_edges.keys().cloned().collect();
        sorted_froms.sort();

        for from_id in sorted_froms {
            let edges = &from_edges[&from_id];
            let from_line = block_map
                .get(&(func_name.clone(), fsl, from_id))
                .map(|(_, line)| *line)
                .unwrap_or(0);

            // Collect throws that go to catch blocks and to exit.
            let catch_edges: Vec<_> = edges
                .iter()
                .filter(|(to, _)| catch_block_ids.contains(to))
                .collect();
            let exit_edges: Vec<_> = edges
                .iter()
                .filter(|(to, _)| exit_block_id == Some(*to))
                .collect();

            // For each exception type seen in the edges from this block,
            // create one throw entry.
            let mut seen_types: Vec<Option<String>> = Vec::new();
            for (_, exc_type) in edges {
                if !seen_types.contains(exc_type) {
                    seen_types.push(exc_type.clone());
                }
            }

            for exc_type in seen_types {
                // Catches that handle this type.
                let caught_by: Vec<u32> = catch_edges
                    .iter()
                    .filter(|(_, et)| et == &exc_type || et.is_none())
                    .map(|(to, _)| *to)
                    .collect();

                let caught_at_lines: Vec<u32> = caught_by
                    .iter()
                    .filter_map(|bid| {
                        block_map
                            .get(&(func_name.clone(), fsl, *bid))
                            .map(|(_, line)| *line)
                    })
                    .collect();

                // If there are exit edges with this type (or no type), it's unhandled.
                let escapes = exit_edges
                    .iter()
                    .any(|(_, et)| et == &exc_type || et.is_none());

                if !caught_by.is_empty() || escapes {
                    throws.push(ThrowEntry {
                        block_id: from_id,
                        line: from_line,
                        exception_type: exc_type.clone(),
                        caught_by,
                        caught_at_lines,
                    });
                }
            }
        }

        // Build catch entries with handled counts.
        let mut catch_entries: Vec<CatchEntry> = catch_block_ids
            .iter()
            .map(|&bid| {
                let catch_line = block_map
                    .get(&(func_name.clone(), fsl, bid))
                    .map(|(_, line)| *line)
                    .unwrap_or(0);

                // Collect all exception types that route to this catch.
                let mut exception_types: Vec<String> = Vec::new();
                for edges in from_edges.values() {
                    for (to, exc_type) in edges {
                        if *to == bid
                            && let Some(t) = exc_type
                            && !exception_types.contains(t)
                        {
                            exception_types.push(t.clone());
                        }
                    }
                }
                exception_types.sort();

                // Count how many throw blocks route to this catch.
                let handled_count =
                    throws.iter().filter(|t| t.caught_by.contains(&bid)).count() as u32;

                CatchEntry {
                    block_id: bid,
                    line: catch_line,
                    exception_types,
                    handled_count,
                }
            })
            .collect();
        catch_entries.sort_by_key(|c| c.line);

        if !throws.is_empty() || !catch_entries.is_empty() {
            functions.push(FunctionExceptions {
                function: func_name,
                function_start_line: fsl,
                throws,
                catches: catch_entries,
            });
        }
    }

    Ok(ExceptionsReport {
        file: file.to_string(),
        functions,
    })
}
