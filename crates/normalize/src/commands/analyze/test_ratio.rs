//! Test/impl ratio: per-module breakdown of test lines vs production lines.
//!
//! Complements `test-gaps` (function-level coverage) with a crate/module-level view.
//! Shows which parts of the codebase have thin or no test coverage by LOC.

use crate::output::OutputFormatter;
use normalize_languages::is_test_path;
use rayon::prelude::*;
use serde::Serialize;
use std::path::Path;

/// Per-module test/impl ratio entry.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestRatioEntry {
    /// Module or crate path (relative to root)
    pub path: String,
    /// Lines of production code
    pub impl_lines: usize,
    /// Lines of test code (dedicated test files + `#[cfg(test)]` blocks)
    pub test_lines: usize,
    /// test_lines / (impl_lines + test_lines), in 0.0–1.0
    pub ratio: f64,
}

/// Report returned by `analyze test-ratio`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TestRatioReport {
    pub root: String,
    pub total_impl_lines: usize,
    pub total_test_lines: usize,
    pub overall_ratio: f64,
    pub entries: Vec<TestRatioEntry>,
}

impl OutputFormatter for TestRatioReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();

        out.push(format!(
            "# Test/Impl Ratio: {} — {:.1}% test coverage by LOC",
            self.root,
            self.overall_ratio * 100.0,
        ));
        out.push(String::new());
        out.push(format!(
            "Total: {} impl lines, {} test lines",
            self.total_impl_lines, self.total_test_lines
        ));
        out.push(String::new());
        out.push(format!(
            "{:<50}  {:>8}  {:>8}  {:>7}",
            "module", "impl", "test", "ratio"
        ));
        out.push("-".repeat(82));

        for e in &self.entries {
            out.push(format!(
                "{:<50}  {:>8}  {:>8}  {:>6.1}%",
                truncate_path(&e.path, 50),
                e.impl_lines,
                e.test_lines,
                e.ratio * 100.0,
            ));
        }

        out.join("\n")
    }
}

fn truncate_path(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - (max - 1)..])
    }
}

pub(crate) struct LineClassification {
    pub(crate) impl_lines: usize,
    pub(crate) test_lines: usize,
}

/// Classify a file as test or impl.
///
/// For Rust files: extract `#[cfg(test)]` block lines as test lines, rest as impl.
/// For other languages: entire file is test if path matches test pattern.
fn classify_file(path: &Path, content: &str) -> LineClassification {
    if is_test_path(path) {
        return LineClassification {
            impl_lines: 0,
            test_lines: content.lines().count(),
        };
    }

    // Rust: extract #[cfg(test)] block lines
    if path.extension().and_then(|e| e.to_str()) == Some("rs") {
        return split_rust_test_lines(content);
    }

    LineClassification {
        impl_lines: content.lines().count(),
        test_lines: 0,
    }
}

/// Split a Rust source file into impl_lines and test_lines by tracking `#[cfg(test)]` mod blocks.
pub(crate) fn split_rust_test_lines(content: &str) -> LineClassification {
    let mut impl_lines = 0usize;
    let mut test_lines = 0usize;
    let mut in_test_block = false;
    let mut brace_depth: i32 = 0;
    let mut test_block_start_depth: i32 = 0;
    let mut pending_cfg_test = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect #[cfg(test)] attribute
        if trimmed == "#[cfg(test)]" || trimmed.starts_with("#[cfg(test)]") {
            pending_cfg_test = true;
            // The attribute line itself is counted as impl (it's boilerplate, not test code)
            impl_lines += 1;
            continue;
        }

        if pending_cfg_test {
            // Next non-empty significant line after #[cfg(test)] should be `mod ... {`
            if trimmed.contains('{') {
                pending_cfg_test = false;
                in_test_block = true;
                test_block_start_depth = brace_depth;
                // Count open braces on this line
                for ch in trimmed.chars() {
                    match ch {
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
                test_lines += 1;
                continue;
            } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
                // Not a block opener — attribute was not for a mod
                pending_cfg_test = false;
            }
        }

        // Track braces for depth
        if in_test_block {
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => brace_depth -= 1,
                    _ => {}
                }
            }
            test_lines += 1;
            // Check if we closed back to before the test block
            if brace_depth <= test_block_start_depth {
                in_test_block = false;
            }
        } else {
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => brace_depth -= 1,
                    _ => {}
                }
            }
            impl_lines += 1;
        }
    }

    LineClassification {
        impl_lines,
        test_lines,
    }
}

/// Analyze test/impl ratio across the codebase.
pub fn analyze_test_ratio(root: &Path, limit: usize) -> TestRatioReport {
    let module_dirs = discover_module_dirs(root);
    let all_files = crate::path_resolve::all_files(root);

    // Collect (relative_path, impl_lines, test_lines)
    let file_data: Vec<(String, usize, usize)> = all_files
        .par_iter()
        .filter(|f| f.kind == "file")
        .filter_map(|f| {
            let abs_path = root.join(&f.path);
            normalize_languages::support_for_path(&abs_path)?;
            let content = std::fs::read_to_string(&abs_path).ok()?;
            if content.is_empty() {
                return None;
            }
            let lc = classify_file(&abs_path, &content);
            if lc.impl_lines + lc.test_lines == 0 {
                return None;
            }
            Some((f.path.clone(), lc.impl_lines, lc.test_lines))
        })
        .collect();

    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (path, impl_lines, test_lines) in &file_data {
        let module = module_key(path, &module_dirs);
        let entry = groups.entry(module).or_default();
        entry.0 += impl_lines;
        entry.1 += test_lines;
    }

    let total_impl_lines: usize = file_data.iter().map(|(_, i, _)| i).sum();
    let total_test_lines: usize = file_data.iter().map(|(_, _, t)| t).sum();
    let overall_ratio = ratio(total_impl_lines, total_test_lines);

    let mut entries: Vec<TestRatioEntry> = groups
        .into_iter()
        .filter(|(_, (impl_l, test_l))| impl_l + test_l > 0)
        .map(|(path, (impl_lines, test_lines))| TestRatioEntry {
            ratio: ratio(impl_lines, test_lines),
            path,
            impl_lines,
            test_lines,
        })
        .collect();

    // Sort: lowest ratio first (least tested at top), then largest impl for ties
    normalize_analyze::ranked::rank_and_truncate(
        &mut entries,
        limit,
        |a, b| {
            a.ratio
                .partial_cmp(&b.ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.impl_lines.cmp(&a.impl_lines))
        },
        |e| e.ratio,
    );

    TestRatioReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        total_impl_lines,
        total_test_lines,
        overall_ratio,
        entries,
    }
}

/// Compute test ratio: test / (impl + test). Returns 0.0 if both zero.
fn ratio(impl_lines: usize, test_lines: usize) -> f64 {
    let total = impl_lines + test_lines;
    if total == 0 {
        0.0
    } else {
        test_lines as f64 / total as f64
    }
}

/// Discover package/module root directories by looking for ecosystem manifest files.
///
/// Queries all registered `LocalDeps` implementations for their manifest filenames
/// (e.g. `Cargo.toml`, `package.json`, `go.mod`) and finds every directory in `root`
/// that contains one. Returns relative paths sorted longest-first so that a caller can
/// find the deepest matching ancestor for any file.
pub(crate) fn discover_module_dirs(root: &Path) -> Vec<String> {
    use normalize_local_deps::registry::all_local_deps;
    use std::collections::{BTreeSet, HashSet};

    let manifests: HashSet<&'static str> = all_local_deps()
        .iter()
        .flat_map(|d| d.project_manifest_filenames().iter().copied())
        .collect();

    if manifests.is_empty() {
        return vec![".".to_string()];
    }

    let all = crate::path_resolve::all_files(root);
    let mut dirs: BTreeSet<String> = BTreeSet::new();

    // Manifest-file based discovery.
    for f in &all {
        let p = std::path::Path::new(&f.path);
        if let Some(name) = p.file_name().and_then(|n| n.to_str())
            && manifests.contains(name)
        {
            let dir = p
                .parent()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default();
            let dir = if dir.is_empty() { ".".to_string() } else { dir };
            dirs.insert(dir);
        }
    }

    // Workspace-member based discovery (e.g. sbt build.sbt, npm workspaces).
    for dep in all_local_deps() {
        for member_path in dep.discover_workspace_members(root) {
            if let Ok(rel) = member_path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy();
                if !rel_str.is_empty() {
                    dirs.insert(rel_str.into_owned());
                }
            }
        }
    }

    dirs.insert(".".to_string());

    let mut result: Vec<String> = dirs.into_iter().collect();
    // Longest first so the most specific (deepest) match wins.
    result.sort_by_key(|b| std::cmp::Reverse(b.len()));
    result
}

/// Map a relative file path to the deepest enclosing package directory.
///
/// `module_dirs` must be sorted longest-first (as returned by `discover_module_dirs`).
pub(crate) fn module_key(path: &str, module_dirs: &[String]) -> String {
    for dir in module_dirs {
        if dir == "." {
            continue;
        }
        let prefix = format!("{}/", dir);
        if path.starts_with(&prefix) || path == dir.as_str() {
            return dir.clone();
        }
    }
    ".".to_string()
}
