//! Test/impl ratio: per-module breakdown of test lines vs production lines.
//!
//! Complements `test-gaps` (function-level coverage) with a crate/module-level view.
//! Shows which parts of the codebase have thin or no test coverage by LOC.

use crate::output::OutputFormatter;
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

/// Classify a file as test or impl and return (impl_lines, test_lines).
///
/// For Rust files: extract `#[cfg(test)]` block lines as test lines, rest as impl.
/// For other languages: entire file is test if path matches test pattern.
fn classify_file(path: &Path, content: &str) -> (usize, usize) {
    let rel = path.to_string_lossy();
    if is_test_file_path(&rel) {
        return (0, content.lines().count());
    }

    // Rust: extract #[cfg(test)] block lines
    if path.extension().and_then(|e| e.to_str()) == Some("rs") {
        let (impl_lines, test_lines) = split_rust_test_lines(content);
        return (impl_lines, test_lines);
    }

    (content.lines().count(), 0)
}

/// Split a Rust source file into (impl_lines, test_lines) by tracking `#[cfg(test)]` mod blocks.
pub(crate) fn split_rust_test_lines(content: &str) -> (usize, usize) {
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

    (impl_lines, test_lines)
}

/// Check whether a file path looks like a dedicated test file.
pub(crate) fn is_test_file_path(path: &str) -> bool {
    let p = path.to_lowercase();
    // Dedicated test directories
    p.starts_with("tests/")
        || p.starts_with("test/")
        || p.contains("/tests/")
        || p.contains("/test/")
        || p.contains("/__tests__/")
        // Rust _test.rs suffix
        || p.ends_with("_test.rs")
        // Go
        || p.ends_with("_test.go")
        // Python
        || p.ends_with("_test.py")
        || p.starts_with("test_")
        || p.contains("/test_")
        // JS/TS
        || p.ends_with(".test.ts")
        || p.ends_with(".test.js")
        || p.ends_with(".test.tsx")
        || p.ends_with(".test.jsx")
        || p.ends_with(".spec.ts")
        || p.ends_with(".spec.js")
        || p.ends_with(".spec.tsx")
        || p.ends_with(".spec.jsx")
}

/// Analyze test/impl ratio across the codebase.
pub fn analyze_test_ratio(root: &Path, limit: usize) -> TestRatioReport {
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
            let (impl_lines, test_lines) = classify_file(&abs_path, &content);
            if impl_lines + test_lines == 0 {
                return None;
            }
            Some((f.path.clone(), impl_lines, test_lines))
        })
        .collect();

    // Group by top-level module (first path component)
    // For each group, sum impl and test lines
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (path, impl_lines, test_lines) in &file_data {
        let module = module_key(path);
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
    entries.sort_by(|a, b| {
        a.ratio
            .partial_cmp(&b.ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.impl_lines.cmp(&a.impl_lines))
    });

    if limit > 0 {
        entries.truncate(limit);
    }

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

/// Collapse a relative path to the top-level crate/module name for grouping.
///
/// "crates/normalize-facts/src/lib.rs" → "crates/normalize-facts"
/// "src/main.rs" → "src"
/// "lib.rs" → "."
pub(crate) fn module_key(path: &str) -> String {
    // Split off first two components if second looks like a crate
    let parts: Vec<&str> = path.splitn(3, '/').collect();
    match parts.as_slice() {
        [top, second, _rest] if *top == "crates" || *top == "packages" => {
            format!("{}/{}", top, second)
        }
        [top, _rest] => top.to_string(),
        [only] => {
            // root-level file
            let _ = only;
            ".".to_string()
        }
        _ => ".".to_string(),
    }
}
