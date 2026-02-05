//! Test gaps analysis command - find untested public functions.
//!
//! Orchestrates: index lookup, symbol extraction, complexity computation,
//! test context classification, call graph analysis, risk scoring.

use crate::analyze::complexity::ComplexityAnalyzer;
use crate::analyze::test_gaps::{FunctionTestGap, TestGapsReport, check_de_priority, compute_risk};
use crate::extract::Extractor;
use crate::filter::Filter;
use crate::path_resolve;
use normalize_languages::{SymbolKind, Visibility, support_for_path};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Intermediate data collected per file during the parallel walk.
struct FileData {
    /// Public functions found in this file
    public_functions: Vec<PublicFunction>,
    /// Names of test functions in this file: (file_path, symbol_name)
    test_symbols: Vec<(String, String)>,
}

/// A public function extracted from a source file.
struct PublicFunction {
    name: String,
    parent: Option<String>,
    file_path: String,
    start_line: usize,
    end_line: usize,
    complexity: usize,
    loc: usize,
}

/// Run test gaps analysis.
pub fn analyze_test_gaps(
    root: &Path,
    target: Option<&str>,
    show_all: bool,
    min_risk: Option<f64>,
    limit: usize,
    filter: Option<&Filter>,
    allowlist: &[String],
) -> TestGapsReport {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    // Step 1: Open index for call graph data
    let index = rt.block_on(crate::index::open(root)).ok();

    // Step 2: Load all call edges into HashMap<callee_name, Vec<(caller_file, caller_symbol)>>
    let callee_to_callers: HashMap<String, Vec<(String, String)>> = if let Some(ref idx) = index {
        let stats = rt.block_on(idx.call_graph_stats()).unwrap_or_default();
        if stats.calls == 0 {
            eprintln!("Warning: Call graph empty or not indexed. Run: normalize index reindex");
            eprintln!("Results will show 0 callers for all functions.");
            HashMap::new()
        } else {
            match rt.block_on(idx.all_call_edges()) {
                Ok(edges) => {
                    let mut map: HashMap<String, Vec<(String, String)>> = HashMap::new();
                    for (caller_file, caller_symbol, callee_name) in edges {
                        map.entry(callee_name)
                            .or_default()
                            .push((caller_file, caller_symbol));
                    }
                    map
                }
                Err(e) => {
                    eprintln!("Warning: Failed to load call graph: {}", e);
                    HashMap::new()
                }
            }
        }
    } else {
        eprintln!("Warning: Index not available. Run: normalize index reindex");
        eprintln!("Results will show 0 callers for all functions.");
        HashMap::new()
    };

    // Step 3: Walk source files, extract symbols + complexity in parallel
    let analysis_root = target
        .map(|t| root.join(t))
        .unwrap_or_else(|| root.to_path_buf());

    let all_files = path_resolve::all_files(&analysis_root);
    let code_files: Vec<_> = all_files
        .iter()
        .filter(|f| f.kind == "file" && support_for_path(Path::new(&f.path)).is_some())
        .filter(|f| {
            filter
                .map(|flt| flt.matches(Path::new(&f.path)))
                .unwrap_or(true)
        })
        .collect();

    let file_data: Vec<FileData> = code_files
        .par_iter()
        .filter_map(|file| {
            let path = analysis_root.join(&file.path);
            let content = std::fs::read_to_string(&path).ok()?;
            let lang = support_for_path(&path)?;

            // Extract symbols with visibility
            let extractor = Extractor::new();
            let result = extractor.extract(&path, &content);

            // Compute complexity per function
            let analyzer = ComplexityAnalyzer::new();
            let complexity_report = analyzer.analyze(&path, &content);
            let complexity_map: HashMap<(String, usize), usize> = complexity_report
                .functions
                .iter()
                .map(|f| ((f.name.clone(), f.start_line), f.complexity))
                .collect();

            let is_test_file = is_test_file_path(&file.path);
            let mut public_functions = Vec::new();
            let mut test_symbols = Vec::new();

            for sym in &result.symbols {
                let is_func = matches!(sym.kind, SymbolKind::Function | SymbolKind::Method);

                // Collect test symbols from this file
                if lang.is_test_symbol(sym) || is_test_file {
                    test_symbols.push((file.path.clone(), sym.name.clone()));
                    // Also mark children as test symbols
                    for child in &sym.children {
                        test_symbols.push((file.path.clone(), child.name.clone()));
                    }
                }

                // Collect public/internal functions (pub + pub(crate) in Rust)
                if is_func
                    && matches!(sym.visibility, Visibility::Public | Visibility::Internal)
                    && !lang.is_test_symbol(sym)
                    && !is_test_file
                {
                    let loc = sym.end_line.saturating_sub(sym.start_line) + 1;
                    let complexity = complexity_map
                        .get(&(sym.name.clone(), sym.start_line))
                        .copied()
                        .unwrap_or(1);

                    public_functions.push(PublicFunction {
                        name: sym.name.clone(),
                        parent: None,
                        file_path: file.path.clone(),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        complexity,
                        loc,
                    });
                }

                // Recurse into children (methods inside impl/class blocks)
                for child in &sym.children {
                    let is_child_func =
                        matches!(child.kind, SymbolKind::Function | SymbolKind::Method);

                    // Collect test symbols from children
                    if lang.is_test_symbol(child) || is_test_file {
                        test_symbols.push((file.path.clone(), child.name.clone()));
                    }

                    if is_child_func
                        && matches!(child.visibility, Visibility::Public | Visibility::Internal)
                        && !lang.is_test_symbol(child)
                        && !is_test_file
                    {
                        let loc = child.end_line.saturating_sub(child.start_line) + 1;
                        let complexity = complexity_map
                            .get(&(child.name.clone(), child.start_line))
                            .copied()
                            .unwrap_or(1);

                        public_functions.push(PublicFunction {
                            name: child.name.clone(),
                            parent: Some(sym.name.clone()),
                            file_path: file.path.clone(),
                            start_line: child.start_line,
                            end_line: child.end_line,
                            complexity,
                            loc,
                        });
                    }
                }
            }

            Some(FileData {
                public_functions,
                test_symbols,
            })
        })
        .collect();

    // Step 4: Merge test symbol sets from all files
    let test_set: HashSet<(String, String)> = file_data
        .iter()
        .flat_map(|fd| fd.test_symbols.iter().cloned())
        .collect();

    // Step 5: For each public function, compute test caller count and risk
    let mut all_gaps: Vec<FunctionTestGap> = file_data
        .into_iter()
        .flat_map(|fd| fd.public_functions)
        .map(|pf| {
            let callers = callee_to_callers
                .get(&pf.name)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            let test_caller_count = callers
                .iter()
                .filter(|(cf, cs)| test_set.contains(&(cf.clone(), cs.clone())))
                .count();

            let caller_count = callers.len();

            let mut risk = if test_caller_count == 0 {
                compute_risk(pf.complexity, caller_count, pf.loc)
            } else {
                0.0
            };

            let de_priority =
                check_de_priority(&pf.name, pf.parent.as_deref(), pf.complexity, pf.loc);

            let (de_prioritized, de_priority_reason) = if let Some(reason) = de_priority {
                risk *= 0.1;
                (true, Some(reason.as_str().to_string()))
            } else {
                (false, None)
            };

            FunctionTestGap {
                name: pf.name,
                parent: pf.parent,
                file_path: pf.file_path,
                start_line: pf.start_line,
                end_line: pf.end_line,
                complexity: pf.complexity,
                caller_count,
                test_caller_count,
                loc: pf.loc,
                risk,
                de_prioritized,
                de_priority_reason,
            }
        })
        .collect();

    // Step 6: Exclude main() entry points (top-level main without parent)
    all_gaps.retain(|g| g.name != "main" || g.parent.is_some());

    // Step 7: Apply allowlist
    let total_before_allow = all_gaps.len();
    if !allowlist.is_empty() {
        all_gaps.retain(|f| {
            let key = f.qualified_name();
            !allowlist.iter().any(|a| key.contains(a))
        });
    }
    let allowed_count = total_before_allow - all_gaps.len();
    let total_public = all_gaps.len();
    let untested_count = all_gaps.iter().filter(|f| f.test_caller_count == 0).count();

    // Step 8: Apply min_risk filter
    if let Some(min) = min_risk {
        all_gaps.retain(|f| f.test_caller_count > 0 || f.risk >= min);
    }

    // Step 9: Sort - untested first by risk desc, then tested by test count asc
    all_gaps.sort_by(|a, b| {
        let a_untested = a.test_caller_count == 0;
        let b_untested = b.test_caller_count == 0;
        match (a_untested, b_untested) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => b
                .risk
                .partial_cmp(&a.risk)
                .unwrap_or(std::cmp::Ordering::Equal),
            (false, false) => a.test_caller_count.cmp(&b.test_caller_count),
        }
    });

    // Step 10: In default mode, only keep untested; truncate
    if !show_all {
        all_gaps.retain(|f| f.test_caller_count == 0);
    }
    all_gaps.truncate(limit);

    TestGapsReport {
        functions: all_gaps,
        total_public,
        untested_count,
        allowed_count,
        show_all,
    }
}

/// Check if a file path is in a test directory/file pattern.
fn is_test_file_path(path: &str) -> bool {
    let p = path.to_lowercase();
    p.starts_with("tests/")
        || p.starts_with("test/")
        || p.contains("/tests/")
        || p.contains("/test/")
        || p.contains("/__tests__/")
        || p.ends_with("_test.go")
        || p.ends_with("_test.rs")
        || p.ends_with(".test.ts")
        || p.ends_with(".test.js")
        || p.ends_with(".spec.ts")
        || p.ends_with(".spec.js")
        || p.ends_with("_test.py")
}
