//! Extract-function recipe: lift a region of code into a new function.
//!
//! Uses CFG liveness data from the index to determine the correct parameter
//! list and return type for the extracted function.
//!
//! # Algorithm
//! 1. Query `cfg_blocks` to find all blocks whose line range overlaps the
//!    selected region.
//! 2. Run a backward-dataflow liveness pass over the *whole function* to
//!    compute `live_in[B]` / `live_out[B]` for every block.
//! 3. Derive:
//!    - **parameters** = ∪(live_in[B] for B in R) ∩ vars_defined_outside_R
//!    - **return vars** = ∪(live_out[B] for B in exit_blocks(R)) ∩ vars_defined_in_R
//! 4. Inspect `cfg_effects` in the region to detect `async`/generator/defer/acquire.
//! 5. Inspect `cfg_edges` for `EdgeKind::Exception` edges that leave the region.
//! 6. Generate language-appropriate source text and produce a `RefactoringPlan`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use crate::{PlannedEdit, RefactoringPlan};

// ─── Public data model ────────────────────────────────────────────────────────

/// Outcome of a successful extract-function plan.
pub struct ExtractFunctionOutcome {
    /// The generated `RefactoringPlan` (new function definition + updated call site).
    pub plan: RefactoringPlan,
    /// User-provided name for the new function.
    pub function_name: String,
    /// Parameters inferred from liveness analysis.
    pub parameters: Vec<Parameter>,
    /// Return type inferred from liveness analysis.
    pub return_type: ReturnType,
    /// Whether the extracted function must be `async`.
    pub is_async: bool,
    /// Whether the extracted function is a generator.
    pub is_generator: bool,
    /// Line number in the original file where the call site will appear
    /// (first line of the extracted region).
    pub call_site_line: u32,
}

/// A parameter of the extracted function.
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    /// Type annotation, if discoverable from the source. `None` means unknown.
    pub inferred_type: Option<String>,
    /// Rust: whether the binding in the original function is `mut`.
    pub mutable: bool,
}

/// Return type of the extracted function.
#[derive(Debug, Clone)]
pub enum ReturnType {
    Unit,
    Single(String),
    Tuple(Vec<String>),
    /// Rust-style `Result<T, E>` when exception edges escape the boundary.
    Result(String, String),
}

/// A warning emitted when the extraction may produce semantically surprising code.
#[derive(Debug, Clone)]
pub enum ExtractionWarning {
    DeferCrossedBoundary {
        line: u32,
    },
    ResourceLifetimeCrossedBoundary {
        label: String,
        acquire_line: u32,
    },
    ExceptionEscapesBoundary {
        exception_type: String,
        throw_line: u32,
    },
    MultipleLiveOutVariables {
        names: Vec<String>,
    },
}

impl ExtractionWarning {
    pub fn to_string_lossy(&self) -> String {
        match self {
            ExtractionWarning::DeferCrossedBoundary { line } => {
                format!(
                    "defer/deferred statement at line {} crosses extraction boundary; defer semantics may not transfer correctly",
                    line
                )
            }
            ExtractionWarning::ResourceLifetimeCrossedBoundary {
                label,
                acquire_line,
            } => {
                format!(
                    "resource '{}' acquired at line {} but not released within extracted region; resource lifetime crosses extraction boundary",
                    label, acquire_line
                )
            }
            ExtractionWarning::ExceptionEscapesBoundary {
                exception_type,
                throw_line,
            } => {
                format!(
                    "exception '{}' thrown at line {} escapes the extraction boundary; the extracted function must declare or handle it",
                    exception_type, throw_line
                )
            }
            ExtractionWarning::MultipleLiveOutVariables { names } => {
                format!(
                    "multiple live-out variables ({}); language may not support multiple return values — consider returning a struct",
                    names.join(", ")
                )
            }
        }
    }
}

// ─── Internal intermediate structs (from index) ───────────────────────────────

#[derive(Debug)]
struct BlockRow {
    block_id: u32,
    #[allow(dead_code)]
    kind: String,
    start_line: u32,
    end_line: u32,
}

#[derive(Debug)]
struct EdgeRow {
    from: u32,
    to: u32,
    kind: String,
    exception_type: Option<String>,
}

#[derive(Debug)]
struct EffectRow {
    block_id: u32,
    kind: String,
    line: u32,
    label: Option<String>,
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Build an extract-function plan without touching the filesystem.
///
/// `file_abs` is the absolute path to the file.
/// `content` is the file's current text.
/// `start_line` and `end_line` are **1-based, inclusive** line numbers selecting
/// the region to extract.
/// `function_name` is the user-provided name for the new function.
pub async fn plan_extract_function(
    ctx: &crate::RefactoringContext,
    file_abs: &Path,
    content: &str,
    start_line: u32,
    end_line: u32,
    function_name: &str,
) -> Result<ExtractFunctionOutcome, String> {
    // ── 0. Resolve the file relative to the project root for index queries ──
    let rel_path = file_abs
        .strip_prefix(&ctx.root)
        .unwrap_or(file_abs)
        .to_string_lossy()
        .to_string();

    // ── 1. Detect grammar / language ──────────────────────────────────────────
    let grammar_name = normalize_languages::support_for_path(file_abs)
        .map(|s| s.name().to_string())
        .unwrap_or_default();

    // ── 2. Query the index ────────────────────────────────────────────────────
    let index = ctx.index.as_ref().ok_or_else(|| {
        "extract-function requires the facts index — run `normalize structure rebuild` first"
            .to_string()
    })?;

    let conn = index.connection();

    // Find the enclosing function: its name and start line.
    let func_row = find_enclosing_function(conn, &rel_path, start_line, end_line).await?;

    let (func_qname, func_start_line) = match func_row {
        Some(r) => r,
        None => {
            return Err(format!(
                "no indexed function found containing lines {}-{} in {}; run `normalize structure rebuild`",
                start_line, end_line, rel_path
            ));
        }
    };

    // Load all blocks for this function.
    let block_rows = load_blocks(conn, &rel_path, &func_qname, func_start_line).await?;
    let edge_rows = load_edges(conn, &rel_path, &func_qname, func_start_line).await?;
    let def_rows = load_defs(conn, &rel_path, &func_qname, func_start_line).await?;
    let use_rows = load_uses(conn, &rel_path, &func_qname, func_start_line).await?;
    let effect_rows = load_effects(conn, &rel_path, &func_qname, func_start_line).await?;

    // ── 3. Identify region blocks ─────────────────────────────────────────────
    let region_block_ids: HashSet<u32> = block_rows
        .iter()
        .filter(|b| b.start_line <= end_line && b.end_line >= start_line)
        .map(|b| b.block_id)
        .collect();

    if region_block_ids.is_empty() {
        return Err(format!(
            "no CFG blocks found overlapping lines {}-{}; the region may be outside any statement",
            start_line, end_line
        ));
    }

    // ── 4. Liveness fixed-point (whole function) ──────────────────────────────
    let block_ids: Vec<u32> = block_rows.iter().map(|b| b.block_id).collect();

    let mut defs: HashMap<u32, BTreeSet<String>> = HashMap::new();
    let mut uses_map: HashMap<u32, BTreeSet<String>> = HashMap::new();
    for id in &block_ids {
        defs.insert(*id, BTreeSet::new());
        uses_map.insert(*id, BTreeSet::new());
    }
    for (bid, name) in &def_rows {
        defs.entry(*bid).or_default().insert(name.clone());
    }
    for (bid, name) in &use_rows {
        uses_map.entry(*bid).or_default().insert(name.clone());
    }

    let mut succs: HashMap<u32, Vec<u32>> = HashMap::new();
    for id in &block_ids {
        succs.insert(*id, Vec::new());
    }
    for e in &edge_rows {
        succs.entry(e.from).or_default().push(e.to);
    }

    let (live_in, live_out) = compute_liveness(&block_ids, &defs, &uses_map, &succs);

    // ── 5. Derive parameters and return vars ──────────────────────────────────
    let vars_defined_in_region: BTreeSet<String> = def_rows
        .iter()
        .filter(|(bid, _)| region_block_ids.contains(bid))
        .map(|(_, name)| name.clone())
        .collect();

    let vars_defined_outside_region: BTreeSet<String> = def_rows
        .iter()
        .filter(|(bid, _)| !region_block_ids.contains(bid))
        .map(|(_, name)| name.clone())
        .collect();

    // Params = live-in of region entry blocks that are defined outside the region.
    // Region entry blocks = region blocks with no predecessors inside the region.
    let region_entry_blocks: Vec<u32> = region_block_ids
        .iter()
        .cloned()
        .filter(|bid| {
            !edge_rows
                .iter()
                .any(|e| e.to == *bid && region_block_ids.contains(&e.from))
        })
        .collect();

    let mut param_names: BTreeSet<String> = BTreeSet::new();
    for bid in &region_entry_blocks {
        if let Some(li) = live_in.get(bid) {
            for v in li {
                if vars_defined_outside_region.contains(v) {
                    param_names.insert(v.clone());
                }
            }
        }
    }

    // Return vars = live-out of region exit blocks that are defined inside the region.
    // Region exit blocks = region blocks whose successors include a block outside the region.
    let region_exit_blocks: Vec<u32> = region_block_ids
        .iter()
        .cloned()
        .filter(|bid| {
            succs
                .get(bid)
                .map(|ss| ss.iter().any(|s| !region_block_ids.contains(s)))
                .unwrap_or(false)
        })
        .collect();

    let mut return_var_names: BTreeSet<String> = BTreeSet::new();
    for bid in &region_exit_blocks {
        if let Some(lo) = live_out.get(bid) {
            for v in lo {
                if vars_defined_in_region.contains(v) {
                    return_var_names.insert(v.clone());
                }
            }
        }
    }

    // ── 6. Build Parameter structs ────────────────────────────────────────────
    // We don't have type information in the index, but we can check if the
    // variable's def site in the source text contains `mut` for Rust.
    let parameters: Vec<Parameter> = param_names
        .iter()
        .map(|name| {
            let mutable = is_mut_binding(&grammar_name, content, name);
            let inferred_type = infer_type_from_annotation(&grammar_name, content, name);
            Parameter {
                name: name.clone(),
                inferred_type,
                mutable,
            }
        })
        .collect();

    // ── 7. Build return type ──────────────────────────────────────────────────
    // Check for escaping exception edges.
    let escaping_exceptions: Vec<(String, u32)> = edge_rows
        .iter()
        .filter(|e| {
            e.kind == "exception"
                && region_block_ids.contains(&e.from)
                && !region_block_ids.contains(&e.to)
        })
        .filter_map(|e| e.exception_type.as_ref().map(|t| (t.clone(), e.from)))
        .collect();

    let return_type = if !escaping_exceptions.is_empty() && grammar_name == "rust" {
        let ret_vars: Vec<String> = return_var_names.iter().cloned().collect();
        let ok_type = if ret_vars.is_empty() {
            "()".to_string()
        } else if ret_vars.len() == 1 {
            ret_vars[0].clone()
        } else {
            format!("({})", ret_vars.join(", "))
        };
        ReturnType::Result(ok_type, "Box<dyn std::error::Error>".to_string())
    } else {
        match return_var_names.len() {
            0 => ReturnType::Unit,
            1 => ReturnType::Single(return_var_names.iter().next().unwrap().clone()),
            _ => {
                let names: Vec<String> = return_var_names.iter().cloned().collect();
                ReturnType::Tuple(names)
            }
        }
    };

    // ── 8. Effects analysis ───────────────────────────────────────────────────
    let region_effects: Vec<&EffectRow> = effect_rows
        .iter()
        .filter(|e| region_block_ids.contains(&e.block_id))
        .collect();

    let is_async = region_effects.iter().any(|e| e.kind == "await");
    let is_generator = region_effects.iter().any(|e| e.kind == "yield");

    // ── 9. Build warnings ─────────────────────────────────────────────────────
    let mut warnings: Vec<ExtractionWarning> = Vec::new();

    for eff in &region_effects {
        if eff.kind == "defer" {
            warnings.push(ExtractionWarning::DeferCrossedBoundary { line: eff.line });
        }
    }

    // Acquire without release in region.
    for eff in &region_effects {
        if eff.kind == "acquire" {
            let label = eff
                .label
                .clone()
                .unwrap_or_else(|| "<resource>".to_string());
            let has_release = region_effects
                .iter()
                .any(|e| e.kind == "release" && e.label == eff.label);
            if !has_release {
                warnings.push(ExtractionWarning::ResourceLifetimeCrossedBoundary {
                    label,
                    acquire_line: eff.line,
                });
            }
        }
    }

    for (exc_type, from_bid) in &escaping_exceptions {
        // Find the throw line from cfg_effects or fall back to block start_line.
        let throw_line = block_rows
            .iter()
            .find(|b| b.block_id == *from_bid)
            .map(|b| b.start_line)
            .unwrap_or(start_line);
        warnings.push(ExtractionWarning::ExceptionEscapesBoundary {
            exception_type: exc_type.clone(),
            throw_line,
        });
    }

    if let ReturnType::Tuple(ref names) = return_type {
        // Some languages don't support multi-return natively.
        if grammar_name != "go"
            && grammar_name != "python"
            && grammar_name != "typescript"
            && grammar_name != "javascript"
        {
            warnings.push(ExtractionWarning::MultipleLiveOutVariables {
                names: names.clone(),
            });
        }
    }

    // ── 10. Determine the extracted region's source text ──────────────────────
    let lines: Vec<&str> = content.lines().collect();
    let region_start_idx = (start_line.saturating_sub(1)) as usize;
    let region_end_idx = (end_line as usize).min(lines.len());

    if region_start_idx >= lines.len() {
        return Err(format!(
            "start line {} is beyond end of file ({} lines)",
            start_line,
            lines.len()
        ));
    }

    let region_lines: Vec<&str> = lines[region_start_idx..region_end_idx].to_vec();

    // Detect indentation of the call site (the first non-empty line of the region).
    let call_site_indent = region_lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            let trimmed = l.trim_start();
            &l[..l.len() - trimmed.len()]
        })
        .unwrap_or("");

    // Normalize the body: strip the common leading whitespace.
    let body_lines = strip_common_indent(&region_lines);
    let body_indent = "    "; // one level of indentation inside the new function

    // ── 11. Generate code ─────────────────────────────────────────────────────
    let new_function = generate_function(
        &grammar_name,
        function_name,
        &parameters,
        &return_type,
        is_async,
        is_generator,
        &body_lines,
        body_indent,
    );

    let call_site = generate_call_site(
        &grammar_name,
        function_name,
        &parameters,
        &return_type,
        is_async,
        call_site_indent,
    );

    // ── 12. Build the PlannedEdit ─────────────────────────────────────────────
    // Replace the extracted lines with the call site.
    // Insert the new function after the enclosing function.
    let new_content = splice_content(content, start_line, end_line, &call_site, &new_function)?;

    let plan = RefactoringPlan {
        operation: "extract_function".to_string(),
        edits: vec![PlannedEdit {
            file: file_abs.to_path_buf(),
            original: content.to_string(),
            new_content,
            description: format!("extract function '{}'", function_name),
        }],
        warnings: warnings.iter().map(|w| w.to_string_lossy()).collect(),
    };

    Ok(ExtractFunctionOutcome {
        plan,
        function_name: function_name.to_string(),
        parameters,
        return_type,
        is_async,
        is_generator,
        call_site_line: start_line,
    })
}

// ─── Index query helpers ──────────────────────────────────────────────────────

async fn find_enclosing_function(
    conn: &libsql::Connection,
    file: &str,
    start_line: u32,
    end_line: u32,
) -> Result<Option<(String, u32)>, String> {
    // Find the function whose block range contains the selection.
    // We pick the innermost function (maximum function_start_line) that spans the region.
    let mut rows = conn
        .query(
            "SELECT function_qname, function_start_line \
             FROM cfg_blocks \
             WHERE file = ?1 \
               AND start_line <= ?2 \
               AND end_line >= ?3 \
             ORDER BY function_start_line DESC \
             LIMIT 1",
            libsql::params![file.to_string(), start_line as i64, end_line as i64],
        )
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    match rows.next().await.map_err(|e| format!("DB error: {}", e))? {
        Some(row) => {
            let qname: String = row.get(0).map_err(|e| format!("DB error: {}", e))?;
            let fsl: i64 = row.get(1).map_err(|e| format!("DB error: {}", e))?;
            Ok(Some((qname, fsl as u32)))
        }
        None => Ok(None),
    }
}

async fn load_blocks(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<BlockRow>, String> {
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
        .map_err(|e| format!("DB error: {}", e))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {}", e))? {
        let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {}", e))?;
        let kind: String = row.get(1).map_err(|e| format!("DB error: {}", e))?;
        let start_line: i64 = row.get(2).map_err(|e| format!("DB error: {}", e))?;
        let end_line: i64 = row.get(3).map_err(|e| format!("DB error: {}", e))?;
        out.push(BlockRow {
            block_id: block_id as u32,
            kind,
            start_line: start_line as u32,
            end_line: end_line as u32,
        });
    }
    Ok(out)
}

async fn load_edges(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<EdgeRow>, String> {
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
        .map_err(|e| format!("DB error: {}", e))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {}", e))? {
        let from: i64 = row.get(0).map_err(|e| format!("DB error: {}", e))?;
        let to: i64 = row.get(1).map_err(|e| format!("DB error: {}", e))?;
        let kind: String = row.get(2).map_err(|e| format!("DB error: {}", e))?;
        let exc: String = row.get(3).map_err(|e| format!("DB error: {}", e))?;
        out.push(EdgeRow {
            from: from as u32,
            to: to as u32,
            kind,
            exception_type: if exc.is_empty() { None } else { Some(exc) },
        });
    }
    Ok(out)
}

async fn load_defs(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<(u32, String)>, String> {
    let mut rows = conn
        .query(
            "SELECT block_id, name \
             FROM cfg_defs \
             WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
            libsql::params![
                file.to_string(),
                func_qname.to_string(),
                func_start_line as i64
            ],
        )
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {}", e))? {
        let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {}", e))?;
        let name: String = row.get(1).map_err(|e| format!("DB error: {}", e))?;
        out.push((block_id as u32, name));
    }
    Ok(out)
}

async fn load_uses(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<(u32, String)>, String> {
    let mut rows = conn
        .query(
            "SELECT block_id, name \
             FROM cfg_uses \
             WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
            libsql::params![
                file.to_string(),
                func_qname.to_string(),
                func_start_line as i64
            ],
        )
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {}", e))? {
        let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {}", e))?;
        let name: String = row.get(1).map_err(|e| format!("DB error: {}", e))?;
        out.push((block_id as u32, name));
    }
    Ok(out)
}

async fn load_effects(
    conn: &libsql::Connection,
    file: &str,
    func_qname: &str,
    func_start_line: u32,
) -> Result<Vec<EffectRow>, String> {
    let mut rows = conn
        .query(
            "SELECT block_id, kind, line, COALESCE(label, '') \
             FROM cfg_effects \
             WHERE file = ?1 AND function_qname = ?2 AND function_start_line = ?3",
            libsql::params![
                file.to_string(),
                func_qname.to_string(),
                func_start_line as i64
            ],
        )
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| format!("DB error: {}", e))? {
        let block_id: i64 = row.get(0).map_err(|e| format!("DB error: {}", e))?;
        let kind: String = row.get(1).map_err(|e| format!("DB error: {}", e))?;
        let line: i64 = row.get(2).map_err(|e| format!("DB error: {}", e))?;
        let label: String = row.get(3).map_err(|e| format!("DB error: {}", e))?;
        out.push(EffectRow {
            block_id: block_id as u32,
            kind,
            line: line as u32,
            label: if label.is_empty() { None } else { Some(label) },
        });
    }
    Ok(out)
}

// ─── Liveness ─────────────────────────────────────────────────────────────────

fn compute_liveness(
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

// ─── Source analysis helpers ──────────────────────────────────────────────────

/// Check whether a variable binding in the source uses `mut` (Rust-specific).
fn is_mut_binding(grammar: &str, content: &str, name: &str) -> bool {
    if grammar != "rust" {
        return false;
    }
    // Heuristic: look for `let mut <name>` in the source.
    // This is a text scan; the index doesn't store mutability.
    let pattern = format!("let mut {}", name);
    content.contains(&pattern)
}

/// Try to extract a type annotation for a variable from the source (best-effort).
/// Returns `None` when no annotation is found.
fn infer_type_from_annotation(grammar: &str, content: &str, name: &str) -> Option<String> {
    match grammar {
        "rust" => {
            // Look for `let [mut] <name>: <type>` or `<name>: <type>` in function params.
            // Simple heuristic: find `<name>: ` and grab the type until `,` or `)` or `=`.
            let pattern = format!("{}: ", name);
            if let Some(pos) = content.find(&pattern) {
                let after = &content[pos + pattern.len()..];
                let end = after.find([',', ')', '=', '\n']).unwrap_or(after.len());
                let ty = after[..end].trim().to_string();
                if !ty.is_empty() && !ty.contains(' ') {
                    return Some(ty);
                }
            }
            None
        }
        "typescript" => {
            // Look for `<name>: <type>` in parameter lists.
            let pattern = format!("{}: ", name);
            if let Some(pos) = content.find(&pattern) {
                let after = &content[pos + pattern.len()..];
                let end = after.find([',', ')', '=', '\n']).unwrap_or(after.len());
                let ty = after[..end].trim().to_string();
                if !ty.is_empty() {
                    return Some(ty);
                }
            }
            None
        }
        _ => None,
    }
}

// ─── Code generation ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn generate_function(
    grammar: &str,
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    is_async: bool,
    is_generator: bool,
    body_lines: &[String],
    indent: &str,
) -> String {
    match grammar {
        "python" => generate_python_function(name, params, ret, is_async, body_lines, indent),
        "go" => generate_go_function(name, params, ret, body_lines, indent),
        "typescript" | "javascript" | "tsx" | "jsx" => {
            generate_ts_function(grammar, name, params, ret, is_async, body_lines, indent)
        }
        "java" => generate_java_function(name, params, ret, body_lines, indent),
        _ => {
            // Default: Rust
            generate_rust_function(
                name,
                params,
                ret,
                is_async,
                is_generator,
                body_lines,
                indent,
            )
        }
    }
}

fn generate_rust_function(
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    is_async: bool,
    _is_generator: bool,
    body_lines: &[String],
    indent: &str,
) -> String {
    let async_kw = if is_async { "async " } else { "" };
    let param_str = params
        .iter()
        .map(|p| {
            let mut_kw = if p.mutable { "mut " } else { "" };
            match &p.inferred_type {
                Some(ty) => format!("{}{}: {}", mut_kw, p.name, ty),
                None => format!("{}{}: /* type */", mut_kw, p.name),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let ret_str = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!(" -> /* {} */", v),
        ReturnType::Tuple(vs) => format!(" -> /* ({}) */", vs.join(", ")),
        ReturnType::Result(ok, err) => format!(" -> Result</* {} */, {}>", ok, err),
    };
    let return_stmt = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!("\n{}    {}", indent, v),
        ReturnType::Tuple(vs) => format!("\n{}    ({})", indent, vs.join(", ")),
        ReturnType::Result(ok, _) => {
            if ok == "()" {
                format!("\n{}    Ok(())", indent)
            } else {
                format!("\n{}    Ok({})", indent, ok)
            }
        }
    };

    let body = body_lines
        .iter()
        .map(|l| format!("{}    {}", indent, l))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n{}{}fn {}({}){} {{\n{}{}\n{}}}\n",
        indent, async_kw, name, param_str, ret_str, body, return_stmt, indent
    )
}

fn generate_python_function(
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    is_async: bool,
    body_lines: &[String],
    indent: &str,
) -> String {
    let async_kw = if is_async { "async " } else { "" };
    let param_str = params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let return_stmt = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!("\n{}    return {}", indent, v),
        ReturnType::Tuple(vs) => format!("\n{}    return {}", indent, vs.join(", ")),
        ReturnType::Result(ok, _) => format!("\n{}    return {}", indent, ok),
    };

    let body = body_lines
        .iter()
        .map(|l| format!("{}    {}", indent, l))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n{}{}def {}({}):\n{}{}\n",
        indent, async_kw, name, param_str, body, return_stmt
    )
}

fn generate_go_function(
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    body_lines: &[String],
    indent: &str,
) -> String {
    // Go: capitalise first letter for exported, lowercase for unexported.
    let param_str = params
        .iter()
        .map(|p| match &p.inferred_type {
            Some(ty) => format!("{} {}", p.name, ty),
            None => format!("{} interface{{}}", p.name),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let ret_str = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!(" /* {} */", v),
        ReturnType::Tuple(vs) => format!(" ({} /* multi-return */)", vs.join(", ")),
        ReturnType::Result(ok, _) => format!(" ({}, error)", ok),
    };
    let return_stmt = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!("\n{}    return {}", indent, v),
        ReturnType::Tuple(vs) => format!("\n{}    return {}", indent, vs.join(", ")),
        ReturnType::Result(ok, _) => format!("\n{}    return {}, nil", indent, ok),
    };

    let body = body_lines
        .iter()
        .map(|l| format!("{}    {}", indent, l))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n{}func {}({}){} {{\n{}{}\n{}}}\n",
        indent, name, param_str, ret_str, body, return_stmt, indent
    )
}

fn generate_ts_function(
    grammar: &str,
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    is_async: bool,
    body_lines: &[String],
    indent: &str,
) -> String {
    let async_kw = if is_async { "async " } else { "" };
    let param_str = params
        .iter()
        .map(|p| match &p.inferred_type {
            Some(ty) if grammar == "typescript" || grammar == "tsx" => {
                format!("{}: {}", p.name, ty)
            }
            _ => p.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");

    let ret_annotation = if grammar == "typescript" || grammar == "tsx" {
        match ret {
            ReturnType::Unit => ": void".to_string(),
            ReturnType::Single(v) => format!(": /* {} */", v),
            ReturnType::Tuple(vs) => format!(
                ": [{}]",
                vs.iter()
                    .map(|v| format!("/* {} */", v))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ReturnType::Result(ok, _) => format!(": {} | Error", ok),
        }
    } else {
        String::new()
    };

    let return_stmt = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!("\n{}    return {};", indent, v),
        ReturnType::Tuple(vs) => format!("\n{}    return [{}];", indent, vs.join(", ")),
        ReturnType::Result(ok, _) => format!("\n{}    return {};", indent, ok),
    };

    let body = body_lines
        .iter()
        .map(|l| format!("{}    {}", indent, l))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n{}{}function {}({}){} {{\n{}{}\n{}}}\n",
        indent, async_kw, name, param_str, ret_annotation, body, return_stmt, indent
    )
}

fn generate_java_function(
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    body_lines: &[String],
    indent: &str,
) -> String {
    let ret_type = match ret {
        ReturnType::Unit => "void".to_string(),
        ReturnType::Single(v) => format!("/* {} */", v),
        ReturnType::Tuple(vs) => format!("/* TODO: struct({}) */", vs.join(", ")),
        ReturnType::Result(ok, err) => format!("/* {} throws {} */", ok, err),
    };
    let param_str = params
        .iter()
        .map(|p| match &p.inferred_type {
            Some(ty) => format!("{} {}", ty, p.name),
            None => format!("/* type */ {}", p.name),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let return_stmt = match ret {
        ReturnType::Unit => String::new(),
        ReturnType::Single(v) => format!("\n{}    return {};", indent, v),
        ReturnType::Tuple(vs) => {
            format!("\n{}    // TODO: return struct({});", indent, vs.join(", "))
        }
        ReturnType::Result(ok, _) => format!("\n{}    return {};", indent, ok),
    };

    let body = body_lines
        .iter()
        .map(|l| format!("{}    {}", indent, l))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n{}private {} {}({}) {{\n{}{}\n{}}}\n",
        indent, ret_type, name, param_str, body, return_stmt, indent
    )
}

fn generate_call_site(
    grammar: &str,
    name: &str,
    params: &[Parameter],
    ret: &ReturnType,
    is_async: bool,
    indent: &str,
) -> String {
    let args = params
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let await_kw = if is_async {
        match grammar {
            "rust" => ".await",
            _ => "await ",
        }
    } else {
        ""
    };

    match grammar {
        "python" => match ret {
            ReturnType::Unit => format!(
                "{}{}{}{}",
                indent,
                await_kw,
                name,
                format_args!("({})", args)
            ),
            ReturnType::Single(v) => {
                format!("{}{} = {}{}({})\n", indent, v, await_kw, name, args)
            }
            ReturnType::Tuple(vs) => {
                format!(
                    "{}{} = {}{}({})\n",
                    indent,
                    vs.join(", "),
                    await_kw,
                    name,
                    args
                )
            }
            ReturnType::Result(ok, _) => format!("{}{} = {}({})\n", indent, ok, name, args),
        },
        "go" => match ret {
            ReturnType::Unit => format!("{}{}({})\n", indent, name, args),
            ReturnType::Single(v) => format!("{}{} := {}({})\n", indent, v, name, args),
            ReturnType::Tuple(vs) => {
                format!("{}{} := {}({})\n", indent, vs.join(", "), name, args)
            }
            ReturnType::Result(ok, _) => {
                format!(
                    "{}{}, err := {}({})\n{}if err != nil {{ return err }}\n",
                    indent, ok, name, args, indent
                )
            }
        },
        "typescript" | "javascript" | "tsx" | "jsx" => match ret {
            ReturnType::Unit => format!(
                "{}{}{}{}",
                indent,
                await_kw,
                name,
                format_args!("({});", args)
            ),
            ReturnType::Single(v) => {
                let prefix = if await_kw == "await " { "await " } else { "" };
                format!("{}const {} = {}{}({});\n", indent, v, prefix, name, args)
            }
            ReturnType::Tuple(vs) => {
                let prefix = if await_kw == "await " { "await " } else { "" };
                format!(
                    "{}const [{}] = {}{}({});\n",
                    indent,
                    vs.join(", "),
                    prefix,
                    name,
                    args
                )
            }
            ReturnType::Result(ok, _) => {
                let prefix = if await_kw == "await " { "await " } else { "" };
                format!("{}const {} = {}{}({});\n", indent, ok, prefix, name, args)
            }
        },
        "java" => match ret {
            ReturnType::Unit => format!("{}{}({});\n", indent, name, args),
            ReturnType::Single(v) => format!("{}var {} = {}({});\n", indent, v, name, args),
            ReturnType::Tuple(vs) => {
                format!(
                    "{}var result = {}({}); // TODO: unpack ({})\n",
                    indent,
                    name,
                    args,
                    vs.join(", ")
                )
            }
            ReturnType::Result(ok, _) => format!("{}var {} = {}({});\n", indent, ok, name, args),
        },
        _ => {
            // Rust
            match ret {
                ReturnType::Unit => {
                    if is_async {
                        format!("{}{}({}).await;\n", indent, name, args)
                    } else {
                        format!("{}{}({});\n", indent, name, args)
                    }
                }
                ReturnType::Single(v) => {
                    if is_async {
                        format!("{}let {} = {}({}).await;\n", indent, v, name, args)
                    } else {
                        format!("{}let {} = {}({});\n", indent, v, name, args)
                    }
                }
                ReturnType::Tuple(vs) => {
                    if is_async {
                        format!(
                            "{}let ({}) = {}({}).await;\n",
                            indent,
                            vs.join(", "),
                            name,
                            args
                        )
                    } else {
                        format!("{}let ({}) = {}({});\n", indent, vs.join(", "), name, args)
                    }
                }
                ReturnType::Result(ok, _) => {
                    if is_async {
                        format!("{}let {} = {}({}).await?;\n", indent, ok, name, args)
                    } else {
                        format!("{}let {} = {}({})?;\n", indent, ok, name, args)
                    }
                }
            }
        }
    }
}

// ─── Content splicing ─────────────────────────────────────────────────────────

/// Strip the common leading whitespace from a set of lines.
fn strip_common_indent(lines: &[&str]) -> Vec<String> {
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|l| {
            if l.len() >= min_indent {
                l[min_indent..].to_string()
            } else {
                l.to_string()
            }
        })
        .collect()
}

/// Replace lines `start_line..=end_line` (1-based) in `content` with `call_site_text`,
/// and append `new_function` after the enclosing function's closing brace.
///
/// Simple strategy:
/// - Replace the region lines with the call site.
/// - Append the new function just after the end of the file (or after the
///   enclosing function). We use end-of-file for simplicity; a future version
///   can find the enclosing function's closing brace.
fn splice_content(
    content: &str,
    start_line: u32,
    end_line: u32,
    call_site: &str,
    new_function: &str,
) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();

    let start_idx = (start_line.saturating_sub(1)) as usize;
    let end_idx = (end_line as usize).min(n);

    if start_idx >= n {
        return Err(format!(
            "start line {} is beyond end of file ({} lines)",
            start_line, n
        ));
    }

    let mut new_lines: Vec<&str> = Vec::new();
    new_lines.extend_from_slice(&lines[..start_idx]);

    // Insert call site lines (drop trailing newline from the generated text).
    let call_site_trimmed = call_site.trim_end_matches('\n');
    for l in call_site_trimmed.lines() {
        new_lines.push(l);
    }

    new_lines.extend_from_slice(&lines[end_idx..]);

    // Preserve trailing newline behaviour.
    let had_trailing = content.ends_with('\n');
    let mut result = new_lines.join("\n");
    if had_trailing {
        result.push('\n');
    }

    // Append the new function at the end of the file.
    result.push_str(new_function);

    Ok(result)
}

// ─── Public helper: parse "start-end" line range ──────────────────────────────

/// Parse a `"start-end"` line-range string (e.g. `"10-25"`) into `(start, end)`.
/// Lines are 1-based inclusive.
pub fn parse_line_range(s: &str) -> Result<(u32, u32), String> {
    match s.split_once('-') {
        Some((a, b)) => {
            let start = a
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid start line in range '{}': expected integer", s))?;
            let end = b
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid end line in range '{}': expected integer", s))?;
            if start == 0 || end == 0 {
                return Err(format!(
                    "line range '{}': lines are 1-based (must be ≥ 1)",
                    s
                ));
            }
            if start > end {
                return Err(format!(
                    "line range '{}': start ({}) must be ≤ end ({})",
                    s, start, end
                ));
            }
            Ok((start, end))
        }
        None => Err(format!(
            "invalid line range '{}': expected 'start-end' (e.g. '10-25')",
            s
        )),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_line_range ──────────────────────────────────────────────────────

    #[test]
    fn parse_line_range_basic() {
        assert_eq!(parse_line_range("10-25").unwrap(), (10, 25));
    }

    #[test]
    fn parse_line_range_single_line() {
        assert_eq!(parse_line_range("5-5").unwrap(), (5, 5));
    }

    #[test]
    fn parse_line_range_rejects_zero() {
        assert!(parse_line_range("0-5").is_err());
    }

    #[test]
    fn parse_line_range_rejects_inverted() {
        assert!(parse_line_range("10-5").is_err());
    }

    #[test]
    fn parse_line_range_rejects_missing_dash() {
        assert!(parse_line_range("10").is_err());
    }

    // ── strip_common_indent ───────────────────────────────────────────────────

    #[test]
    fn strip_common_indent_basic() {
        let lines = vec!["    let x = 1;", "    let y = 2;"];
        let out = strip_common_indent(&lines);
        assert_eq!(out, vec!["let x = 1;", "let y = 2;"]);
    }

    #[test]
    fn strip_common_indent_mixed() {
        let lines = vec!["    let x = 1;", "        let y = 2;"];
        let out = strip_common_indent(&lines);
        assert_eq!(out, vec!["let x = 1;", "    let y = 2;"]);
    }

    // ── generate_rust_function ────────────────────────────────────────────────

    #[test]
    fn generate_rust_fn_unit_return() {
        let params = vec![Parameter {
            name: "x".to_string(),
            inferred_type: Some("i32".to_string()),
            mutable: false,
        }];
        let body = vec!["println!(\"{}\", x);".to_string()];
        let out = generate_rust_function(
            "do_thing",
            &params,
            &ReturnType::Unit,
            false,
            false,
            &body,
            "",
        );
        assert!(out.contains("fn do_thing(x: i32)"));
        assert!(out.contains("println!"));
        assert!(!out.contains("->"));
    }

    #[test]
    fn generate_rust_fn_single_return() {
        let params = vec![];
        let body = vec!["let result = 42;".to_string()];
        let out = generate_rust_function(
            "compute",
            &params,
            &ReturnType::Single("result".to_string()),
            false,
            false,
            &body,
            "",
        );
        assert!(out.contains("fn compute()"));
        assert!(out.contains("-> /* result */"));
        assert!(out.contains("result"));
    }

    #[test]
    fn generate_rust_fn_async() {
        let params = vec![];
        let body = vec!["tokio::time::sleep(Duration::from_secs(1)).await;".to_string()];
        let out = generate_rust_function(
            "wait_a_bit",
            &params,
            &ReturnType::Unit,
            true,
            false,
            &body,
            "",
        );
        assert!(out.contains("async fn wait_a_bit()"));
    }

    // ── generate_python_function ──────────────────────────────────────────────

    #[test]
    fn generate_python_fn_basic() {
        let params = vec![Parameter {
            name: "x".to_string(),
            inferred_type: None,
            mutable: false,
        }];
        let body = vec!["print(x)".to_string()];
        let out = generate_python_function("show", &params, &ReturnType::Unit, false, &body, "");
        assert!(out.contains("def show(x):"));
        assert!(out.contains("print(x)"));
    }

    #[test]
    fn generate_python_fn_multi_return() {
        let params = vec![];
        let body = vec!["a = 1".to_string(), "b = 2".to_string()];
        let out = generate_python_function(
            "two_values",
            &params,
            &ReturnType::Tuple(vec!["a".to_string(), "b".to_string()]),
            false,
            &body,
            "",
        );
        assert!(out.contains("return a, b"));
    }

    // ── generate_go_function ──────────────────────────────────────────────────

    #[test]
    fn generate_go_fn_basic() {
        let params = vec![Parameter {
            name: "n".to_string(),
            inferred_type: Some("int".to_string()),
            mutable: false,
        }];
        let body = vec!["result := n * 2".to_string()];
        let out = generate_go_function(
            "double",
            &params,
            &ReturnType::Single("result".to_string()),
            &body,
            "",
        );
        assert!(out.contains("func double(n int)"));
        assert!(out.contains("return result"));
    }

    // ── splice_content ────────────────────────────────────────────────────────

    #[test]
    fn splice_content_replaces_region() {
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let result =
            splice_content(content, 2, 3, "    call()\n", "\nfn extracted() {}\n").unwrap();
        assert!(result.contains("line1\n    call()\nline4\nline5\n"));
        assert!(result.contains("fn extracted()"));
    }

    #[test]
    fn splice_content_preserves_surrounding_lines() {
        let content = "a\nb\nc\nd\n";
        let result = splice_content(content, 2, 2, "X\n", "\nfn f() {}\n").unwrap();
        assert!(result.starts_with("a\nX\nc\nd\n"));
    }

    // ── call site generation ──────────────────────────────────────────────────

    #[test]
    fn rust_call_site_with_return() {
        let params = vec![Parameter {
            name: "x".to_string(),
            inferred_type: Some("i32".to_string()),
            mutable: false,
        }];
        let call = generate_call_site(
            "rust",
            "compute",
            &params,
            &ReturnType::Single("result".to_string()),
            false,
            "    ",
        );
        assert_eq!(call, "    let result = compute(x);\n");
    }

    #[test]
    fn rust_call_site_async() {
        let params = vec![];
        let call = generate_call_site(
            "rust",
            "fetch",
            &params,
            &ReturnType::Single("data".to_string()),
            true,
            "    ",
        );
        assert_eq!(call, "    let data = fetch().await;\n");
    }

    #[test]
    fn python_call_site_multi_return() {
        let params = vec![];
        let call = generate_call_site(
            "python",
            "two_vals",
            &params,
            &ReturnType::Tuple(vec!["a".to_string(), "b".to_string()]),
            false,
            "",
        );
        assert_eq!(call, "a, b = two_vals()\n");
    }
}
