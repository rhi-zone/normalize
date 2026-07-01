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

use normalize_facts::cfg_dataflow::{self, CfgBlockRow, CfgEdgeRow};

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

    // ── 1. Detect language and its code-generation capability ─────────────────
    let support = normalize_languages::support_for_path(file_abs).ok_or_else(|| {
        format!(
            "extract-function: no language support for {}",
            file_abs.display()
        )
    })?;
    let cg = support.as_refactor_codegen().ok_or_else(|| {
        format!(
            "extract-function does not support language {} (no code generation implemented)",
            support.name()
        )
    })?;

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

    // Load all blocks for this function (cfg_* tables owned by normalize-facts).
    let block_rows: Vec<CfgBlockRow> =
        cfg_dataflow::load_blocks(conn, &rel_path, &func_qname, func_start_line).await?;
    let edge_rows: Vec<CfgEdgeRow> =
        cfg_dataflow::load_edges(conn, &rel_path, &func_qname, func_start_line).await?;
    let def_rows = cfg_dataflow::load_defs(conn, &rel_path, &func_qname, func_start_line).await?;
    let use_rows = cfg_dataflow::load_uses(conn, &rel_path, &func_qname, func_start_line).await?;
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

    let (live_in, live_out) = cfg_dataflow::compute_liveness(&block_ids, &defs, &uses_map, &succs);

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
            let mutable = cg.param_is_mutable(content, name);
            let inferred_type = cg.infer_param_type(content, name);
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

    let return_type = if !escaping_exceptions.is_empty() && cg.uses_result_for_exceptions() {
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
        if !cg.supports_multi_return() {
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

    // ── 11. Generate code (via the language's RefactorCodeGen) ────────────────
    let gen_params = to_gen_params(&parameters);
    let new_function = cg.render_function(&normalize_languages::ExtractedFnSpec {
        name: function_name.to_string(),
        params: gen_params.clone(),
        ret: to_gen_return(&return_type),
        is_async,
        is_generator,
        body_lines: body_lines.clone(),
        indent: body_indent.to_string(),
    });

    let call_site = cg.render_call_site(&normalize_languages::CallSiteSpec {
        name: function_name.to_string(),
        params: gen_params,
        ret: to_gen_return(&return_type),
        is_async,
        indent: call_site_indent.to_string(),
    });

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

// ─── Spec mapping (recipe types → normalize-languages codegen inputs) ──────────

/// Map the recipe's `Parameter` list into the language-agnostic `GenParam` inputs
/// that `RefactorCodeGen` consumes.
fn to_gen_params(params: &[Parameter]) -> Vec<normalize_languages::GenParam> {
    params
        .iter()
        .map(|p| normalize_languages::GenParam {
            name: p.name.clone(),
            inferred_type: p.inferred_type.clone(),
            mutable: p.mutable,
        })
        .collect()
}

/// Map the recipe's `ReturnType` into the codegen `GenReturn`.
fn to_gen_return(ret: &ReturnType) -> normalize_languages::GenReturn {
    use normalize_languages::GenReturn;
    match ret {
        ReturnType::Unit => GenReturn::Unit,
        ReturnType::Single(v) => GenReturn::Single(v.clone()),
        ReturnType::Tuple(vs) => GenReturn::Tuple(vs.clone()),
        ReturnType::Result(ok, err) => GenReturn::Result(ok.clone(), err.clone()),
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
}
