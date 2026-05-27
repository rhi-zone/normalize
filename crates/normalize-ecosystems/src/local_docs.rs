//! Local documentation extraction for Rust (Cargo) packages.
//!
//! Implements [`LocalDocsExtractor`] by resolving packages via `cargo metadata`
//! and parsing doc-comment lines (`///` and `//!`) from source files.
//!
//! No network access is performed; all information comes from the on-disk
//! Cargo registry cache (`~/.cargo/registry/src/...`) populated by prior
//! `cargo build` / `cargo fetch` runs.

use crate::{DocsError, LocalDocsExtractor, symbol_docs::SymbolDoc};
use std::path::{Path, PathBuf};

// ── public extractor ─────────────────────────────────────────────────────────

/// Local docs extractor for Rust / Cargo packages.
///
/// Resolves source via `cargo metadata`, then walks module files looking for
/// the requested symbol and extracting its attached doc comments.
pub struct CargoLocalDocsExtractor {
    /// Directory to run `cargo metadata` in (should contain Cargo.toml).
    pub project_root: PathBuf,
}

impl CargoLocalDocsExtractor {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }
}

impl LocalDocsExtractor for CargoLocalDocsExtractor {
    fn extract_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<SymbolDoc, DocsError> {
        extract_local(package, symbol_path, version, &self.project_root)
    }
}

// ── implementation ────────────────────────────────────────────────────────────

fn extract_local(
    package: &str,
    symbol_path: &str,
    version: Option<&str>,
    project_root: &Path,
) -> Result<SymbolDoc, DocsError> {
    // 1. Resolve source directory via cargo metadata
    let src_dir = resolve_source_dir(package, version, project_root)?;

    // 2. Parse symbol path into (module_parts, item_name)
    //    e.g. "serde::Serialize" -> ([], "Serialize")
    //         "tokio::sync::Mutex" -> (["sync"], "Mutex")
    //         "serde" -> crate root
    let (module_parts, item_name) = split_symbol_path(package, symbol_path);

    if let Some(name) = item_name {
        extract_item_docs(package, symbol_path, &src_dir, &module_parts, &name)
    } else {
        extract_crate_root_docs(package, symbol_path, &src_dir)
    }
}

/// Run `cargo metadata` and locate the source directory for `package@version`.
fn resolve_source_dir(
    package: &str,
    version: Option<&str>,
    project_root: &Path,
) -> Result<PathBuf, DocsError> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(project_root)
        .output()
        .map_err(|e| DocsError::ToolFailed(format!("cargo metadata failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DocsError::ToolFailed(format!(
            "cargo metadata error: {}",
            stderr.trim()
        )));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| DocsError::ParseError(format!("invalid cargo metadata JSON: {}", e)))?;

    let packages = json
        .get("packages")
        .and_then(|p| p.as_array())
        .ok_or_else(|| DocsError::ParseError("missing packages array in metadata".to_string()))?;

    // Find the package matching name (and version if specified)
    for pkg in packages {
        let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name != package {
            continue;
        }
        if let Some(req_ver) = version {
            let pkg_ver = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("");
            if pkg_ver != req_ver {
                continue;
            }
        }
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| DocsError::ParseError("missing manifest_path".to_string()))?;
        let src_dir = PathBuf::from(manifest_path)
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| {
                DocsError::ParseError(format!("invalid manifest_path: {}", manifest_path))
            })?;

        let src = src_dir.join("src");
        if src.is_dir() {
            return Ok(src);
        }
        return Ok(src_dir);
    }

    Err(DocsError::NotFound(format!(
        "package '{}' not found in cargo metadata (not a dependency or not yet fetched)",
        package
    )))
}

/// Extract crate-root docs from `src/lib.rs` (or `src/main.rs`).
fn extract_crate_root_docs(
    package: &str,
    symbol_path: &str,
    src_dir: &Path,
) -> Result<SymbolDoc, DocsError> {
    let candidates = ["lib.rs", "main.rs"];
    for name in &candidates {
        let path = src_dir.join(name);
        if path.is_file() {
            let content = std::fs::read_to_string(&path).map_err(|e| {
                DocsError::ToolFailed(format!("failed to read {}: {}", path.display(), e))
            })?;
            let doc_text = extract_module_doc_comments(&content);
            // Resolved version from manifest
            let version = read_manifest_version(src_dir);
            let source_url = format!(
                "https://docs.rs/{}/{}/{}/index.html",
                package,
                version.as_deref().unwrap_or("latest"),
                package.replace('-', "_")
            );
            return Ok(SymbolDoc {
                name: package.to_string(),
                language: "rust".to_string(),
                package: package.to_string(),
                version: version.unwrap_or_default(),
                symbol_path: symbol_path.to_string(),
                kind: "module".to_string(),
                signature: None,
                doc_text,
                examples: vec![],
                source_url,
                fetched_at: chrono::Utc::now(),
            });
        }
    }
    Err(DocsError::NotFound(format!(
        "no lib.rs or main.rs found in {}",
        src_dir.display()
    )))
}

/// Extract docs for a named item (trait, struct, fn, enum, type, ...).
fn extract_item_docs(
    package: &str,
    symbol_path: &str,
    src_dir: &Path,
    module_parts: &[String],
    item_name: &str,
) -> Result<SymbolDoc, DocsError> {
    let version = read_manifest_version(src_dir);

    // 1. Try targeted module files first (fast path)
    let candidate_files = module_candidate_files(src_dir, module_parts);
    for file in &candidate_files {
        if !file.is_file() {
            continue;
        }
        let content = std::fs::read_to_string(file).map_err(|e| {
            DocsError::ToolFailed(format!("failed to read {}: {}", file.display(), e))
        })?;

        if let Some((kind, sig, doc_text, examples)) = extract_item_from_source(&content, item_name)
        {
            let source_url = build_docs_rs_url(
                package,
                version.as_deref().unwrap_or("latest"),
                module_parts,
                &kind,
                item_name,
            );
            return Ok(SymbolDoc {
                name: item_name.to_string(),
                language: "rust".to_string(),
                package: package.to_string(),
                version: version.unwrap_or_default(),
                symbol_path: symbol_path.to_string(),
                kind,
                signature: sig,
                doc_text,
                examples,
                source_url,
                fetched_at: chrono::Utc::now(),
            });
        }
    }

    // 2. Fallback: recursively search all .rs files in the source tree.
    //    This handles re-exported items (e.g. serde::Serialize is re-exported
    //    from src/lib.rs but defined in src/core/ser/mod.rs).
    if let Some(result) = search_all_source_files(src_dir, item_name) {
        let (kind, sig, doc_text, examples) = result;
        let source_url = build_docs_rs_url(
            package,
            version.as_deref().unwrap_or("latest"),
            module_parts,
            &kind,
            item_name,
        );
        return Ok(SymbolDoc {
            name: item_name.to_string(),
            language: "rust".to_string(),
            package: package.to_string(),
            version: version.unwrap_or_default(),
            symbol_path: symbol_path.to_string(),
            kind,
            signature: sig,
            doc_text,
            examples,
            source_url,
            fetched_at: chrono::Utc::now(),
        });
    }

    Err(DocsError::NotFound(format!(
        "symbol '{}' not found in local source for '{}'",
        item_name, package
    )))
}

/// Recursively walk all `.rs` files under `src_dir` looking for `item_name`.
/// Returns the first match found.
// normalize-syntax-allow: rust/tuple-return - private parsing helper, struct overhead unwarranted
fn search_all_source_files(
    src_dir: &Path,
    item_name: &str,
) -> Option<(String, Option<String>, String, Vec<String>)> {
    walk_rs_files(src_dir, item_name)
}

fn walk_rs_files(
    dir: &Path,
    item_name: &str,
) -> Option<(String, Option<String>, String, Vec<String>)> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
            files.push(path);
        } else if path.is_dir() {
            dirs.push(path);
        }
    }

    // Files first, then recurse into subdirectories
    for file in files {
        if let Ok(content) = std::fs::read_to_string(&file) {
            if let Some(result) = extract_item_from_source(&content, item_name) {
                return Some(result);
            }
        }
    }
    for sub_dir in dirs {
        if let Some(result) = walk_rs_files(&sub_dir, item_name) {
            return Some(result);
        }
    }
    None
}

/// Build the list of source files to search for a given module path.
/// For `module_parts = ["sync"]`, returns `src/sync.rs`, `src/sync/mod.rs`, etc.
fn module_candidate_files(src_dir: &Path, module_parts: &[String]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if module_parts.is_empty() {
        // Item lives in the crate root
        candidates.push(src_dir.join("lib.rs"));
        candidates.push(src_dir.join("main.rs"));
    } else {
        let mut prefix = src_dir.to_path_buf();
        // Walk into the module path
        for (i, part) in module_parts.iter().enumerate() {
            if i == module_parts.len() - 1 {
                // Last segment: could be file.rs or dir/mod.rs
                candidates.push(prefix.join(format!("{}.rs", part)));
                candidates.push(prefix.join(part).join("mod.rs"));
            }
            prefix = prefix.join(part);
        }
    }

    // Always also search lib.rs / mod.rs (re-exports are common)
    candidates.push(src_dir.join("lib.rs"));
    candidates.dedup();
    candidates
}

/// Read the version from a `Cargo.toml` adjacent to `src/`.
fn read_manifest_version(src_dir: &Path) -> Option<String> {
    // src_dir is typically `<crate_root>/src`
    let manifest = src_dir.parent()?.join("Cargo.toml");
    let content = std::fs::read_to_string(manifest).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    parsed
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Build a docs.rs URL for a symbol.
fn build_docs_rs_url(
    package: &str,
    version: &str,
    module_parts: &[String],
    kind: &str,
    item_name: &str,
) -> String {
    let crate_slug = package.replace('-', "_");
    let module_segment = if module_parts.is_empty() {
        String::new()
    } else {
        format!("{}/", module_parts.join("/"))
    };
    format!(
        "https://docs.rs/{}/{}/{}/{}{}.{}.html",
        package, version, crate_slug, module_segment, kind, item_name
    )
}

// ── Source parsing helpers ─────────────────────────────────────────────────

/// Extract `//!` module-level doc comments from a file.
fn extract_module_doc_comments(src: &str) -> String {
    let lines: Vec<&str> = src
        .lines()
        .take_while(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("//!") || trimmed.starts_with("//") || trimmed.is_empty()
        })
        .filter(|line| line.trim().starts_with("//!"))
        .collect();

    if lines.is_empty() {
        return String::new();
    }

    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed == "//!" {
                ""
            } else if trimmed.starts_with("//! ") {
                &trimmed[4..]
            } else {
                &trimmed[3..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Extract doc comments, kind, signature, and examples for `item_name` in `src`.
///
/// Returns `Some((kind, signature, doc_text, examples))` if found, else `None`.
// normalize-syntax-allow: rust/tuple-return - private parsing helper, struct overhead unwarranted
fn extract_item_from_source(
    src: &str,
    item_name: &str,
) -> Option<(String, Option<String>, String, Vec<String>)> {
    let lines: Vec<&str> = src.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Look for item declarations: `pub trait Foo`, `pub struct Foo`, etc.
        // Pass the original (untrimmed) line so indentation checks work correctly.
        if let Some((kind, sig)) = match_item_declaration(line, item_name) {
            let _ = trimmed; // trimmed unused now
            // Collect doc comments immediately preceding this line
            let doc_text = collect_preceding_doc_comments(&lines, i);
            let examples = extract_examples_from_doc(&doc_text);
            // Strip example blocks from doc_text to avoid duplication
            let clean_doc = strip_example_blocks(&doc_text);
            return Some((kind, Some(sig), clean_doc, examples));
        }
    }

    None
}

/// Match a line like `pub trait Foo`, `pub struct Foo`, `pub fn foo_bar`, etc.
/// Only matches items at the crate/module top level (no indentation).
/// `line` is the original (possibly indented) source line.
/// Returns `(kind, trimmed_signature_line)` if matched.
// normalize-syntax-allow: rust/tuple-return - private parsing helper, struct overhead unwarranted
fn match_item_declaration(line: &str, item_name: &str) -> Option<(String, String)> {
    // Only match items at the top level — skip indented code (inside impls, macros, etc.)
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }
    let trimmed = line.trim();
    // Strip visibility and attributes noise
    let stripped = strip_leading_pub(trimmed);

    // Patterns to match
    let patterns: &[(&str, &str)] = &[
        ("trait ", "trait"),
        ("struct ", "struct"),
        ("enum ", "enum"),
        ("fn ", "fn"),
        ("type ", "type"),
        ("const ", "constant"),
        ("static ", "constant"),
        ("macro_rules! ", "macro"),
        ("macro ", "macro"),
        ("mod ", "module"),
    ];

    for (prefix, kind) in patterns {
        if let Some(rest) = stripped.strip_prefix(prefix) {
            // item name is the first token
            let name_end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            let found_name = &rest[..name_end];
            if found_name == item_name {
                return Some((kind.to_string(), trimmed.to_string()));
            }
        }
    }

    None
}

fn strip_leading_pub(s: &str) -> &str {
    let s = s.trim();
    // pub(crate) pub(super) pub(in ...) pub
    if s.starts_with("pub(") {
        if let Some(end) = s.find(')') {
            return s[end + 1..].trim();
        }
    }
    s.strip_prefix("pub ").map(str::trim).unwrap_or(s)
}

/// Walk backwards from `line_index` collecting `///` doc comment lines.
///
/// Skips over attribute blocks (`#[...]`, including multi-line ones) so that
/// attributes between doc comments and the item declaration are transparent.
fn collect_preceding_doc_comments(lines: &[&str], line_index: usize) -> String {
    let mut comments: Vec<String> = Vec::new();

    // Start one line above the declaration
    let mut i = match line_index.checked_sub(1) {
        Some(v) => v,
        None => return String::new(),
    };

    loop {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("/// ") {
            comments.push(trimmed[4..].to_string());
        } else if trimmed == "///" {
            comments.push(String::new());
        } else if trimmed.starts_with("//!") {
            // Module-level docs encountered — stop
            break;
        } else if is_in_attribute_block(lines, i) {
            // Jump to the line above the #[ that starts this attribute block.
            // skip_attribute_block_upward returns the index of the #[ line itself,
            // so we want to continue from (that index - 1) on the next iteration.
            let attr_start = find_attribute_block_start(lines, i);
            if attr_start == 0 {
                break;
            }
            i = attr_start - 1;
            // Don't fall through to the `i -= 1` at the bottom
            continue;
        } else if trimmed.is_empty() {
            if !comments.is_empty() {
                comments.push(String::new()); // paragraph break
            }
            // Keep walking up
        } else {
            // Non-doc, non-attribute, non-blank: stop
            break;
        }

        if i == 0 {
            break;
        }
        i -= 1;
    }

    // Remove trailing blank entries from walking too far up
    while comments
        .last()
        .map(|s: &String| s.is_empty())
        .unwrap_or(false)
    {
        comments.pop();
    }

    comments.reverse();
    comments.join("\n").trim().to_string()
}

/// Returns true if line `i` is part of a `#[...]` or `#![...]` attribute block
/// (which may span multiple lines).
fn is_in_attribute_block(lines: &[&str], i: usize) -> bool {
    let trimmed = lines[i].trim();
    // Starts with #[ or #![ — beginning of an attribute
    if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
        return true;
    }
    // Could be a continuation line of a multi-line attribute.
    // We detect this by walking forward from a `#[` start to see if `i` is inside.
    // For simplicity: if the line doesn't look like a doc comment, code, or blank,
    // and contains content that looks like attribute innards, treat it as attribute.
    // A line inside `#[cfg_attr(...)]` may be `    not(no_diagnostic_namespace),`.
    // We use a heuristic: search upward for a `#[` that hasn't been closed yet.
    let mut depth = 0i32;
    // Scan from position i upward, counting brackets
    let mut j = i;
    loop {
        let t = lines[j].trim();
        // Count brackets in this line (backwards: we're scanning up)
        for c in t.chars().rev() {
            match c {
                ']' => depth += 1,
                '[' => depth -= 1,
                _ => {}
            }
        }
        if t.starts_with("#[") || t.starts_with("#![") {
            return depth >= 0; // we found an opening #[ that owns line i
        }
        // If we hit something that's clearly not an attribute, stop
        if t.starts_with("///")
            || t.starts_with("//!")
            || t.starts_with("pub ")
            || t.starts_with("fn ")
            || t.starts_with("struct ")
            || t.starts_with("trait ")
            || t.starts_with("enum ")
        {
            break;
        }
        if j == 0 {
            break;
        }
        j -= 1;
    }
    false
}

/// Given a line index known to be inside or at the start of an attribute block,
/// return the index of the `#[` line that starts the block.
fn find_attribute_block_start(lines: &[&str], i: usize) -> usize {
    let mut j = i;
    loop {
        let t = lines[j].trim();
        if t.starts_with("#[") || t.starts_with("#![") {
            return j;
        }
        if j == 0 {
            return 0;
        }
        j -= 1;
    }
}

/// Extract ```rust ... ``` example blocks from a doc string.
fn extract_examples_from_doc(doc: &str) -> Vec<String> {
    let mut examples = Vec::new();
    let mut in_block = false;
    let mut block_lines: Vec<&str> = Vec::new();

    for line in doc.lines() {
        let trimmed = line.trim();
        if !in_block && (trimmed.starts_with("```rust") || trimmed == "```") {
            in_block = true;
            block_lines.clear();
        } else if in_block && trimmed == "```" {
            in_block = false;
            let block = block_lines.join("\n").trim().to_string();
            if !block.is_empty() {
                examples.push(block);
            }
            block_lines.clear();
        } else if in_block {
            block_lines.push(line);
        }
    }

    examples
}

/// Remove ```rust ... ``` blocks from doc text (after extracting examples).
fn strip_example_blocks(doc: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;

    for line in doc.lines() {
        let trimmed = line.trim();
        if !in_block && (trimmed.starts_with("```rust") || trimmed == "```") {
            in_block = true;
        } else if in_block && trimmed == "```" {
            in_block = false;
        } else if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }

    out.trim().to_string()
}

/// Split "serde::Serialize" into ([], "Serialize"),
/// "tokio::sync::Mutex" into (["sync"], "Mutex"),
/// "serde" into ([], None) (crate root).
// normalize-syntax-allow: rust/tuple-return - private parsing helper, struct overhead unwarranted
fn split_symbol_path(package: &str, symbol_path: &str) -> (Vec<String>, Option<String>) {
    if symbol_path == package || symbol_path == &format!("{}::", package) {
        return (vec![], None);
    }

    let stripped = if symbol_path.starts_with(&format!("{}::", package)) {
        &symbol_path[package.len() + 2..]
    } else {
        symbol_path
    };

    if stripped.is_empty() {
        return (vec![], None);
    }

    let parts: Vec<&str> = stripped.split("::").collect();
    if parts.len() == 1 {
        (vec![], Some(parts[0].to_string()))
    } else {
        let module = parts[..parts.len() - 1]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let item = parts[parts.len() - 1].to_string();
        (module, Some(item))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_symbol_path_crate_root() {
        let (m, i) = split_symbol_path("serde", "serde");
        assert!(m.is_empty());
        assert!(i.is_none());
    }

    #[test]
    fn test_split_symbol_path_top_level() {
        let (m, i) = split_symbol_path("serde", "serde::Serialize");
        assert!(m.is_empty());
        assert_eq!(i.as_deref(), Some("Serialize"));
    }

    #[test]
    fn test_split_symbol_path_nested() {
        let (m, i) = split_symbol_path("tokio", "tokio::sync::Mutex");
        assert_eq!(m, vec!["sync"]);
        assert_eq!(i.as_deref(), Some("Mutex"));
    }

    #[test]
    fn test_extract_module_doc_comments() {
        let src = "//! # My crate\n//!\n//! Does things.\n\npub fn main() {}";
        let doc = extract_module_doc_comments(src);
        assert!(doc.contains("My crate"));
        assert!(doc.contains("Does things."));
    }

    #[test]
    fn test_extract_item_from_source_trait() {
        let src = r#"
/// A data structure that can be serialized.
///
/// Implement this to make your type serializable.
pub trait Serialize {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>;
}
"#;
        let result = extract_item_from_source(src, "Serialize");
        assert!(result.is_some());
        let (kind, sig, doc, _examples) = result.unwrap();
        assert_eq!(kind, "trait");
        assert!(sig.unwrap().contains("Serialize"));
        assert!(doc.contains("serialized"));
    }

    #[test]
    fn test_extract_examples() {
        let doc = "Do stuff.\n\n```rust\nlet x = 1;\n```\n\nMore text.";
        let examples = extract_examples_from_doc(doc);
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0], "let x = 1;");
    }

    #[test]
    fn test_match_item_declaration_struct() {
        let result = match_item_declaration("pub struct Foo {", "Foo");
        assert!(result.is_some());
        let (kind, _) = result.unwrap();
        assert_eq!(kind, "struct");
    }

    #[test]
    fn test_match_item_declaration_no_match() {
        assert!(match_item_declaration("pub struct Bar {", "Foo").is_none());
    }

    #[test]
    fn test_collect_doc_comments_with_multiline_attr() {
        // Simulate the serde Serialize trait declaration with a multi-line #[cfg_attr]
        // that contains // comments inside it (like the actual serde source)
        let src = concat!(
            "/// A **data structure** that can be serialized.\n",
            "///\n",
            "/// Serde provides `Serialize` implementations for many types.\n",
            "///\n",
            "/// [derive section of the manual]: https://serde.rs/derive.html\n",
            "#[cfg_attr(\n",
            "    not(no_diagnostic_namespace),\n",
            "    diagnostic::on_unimplemented(\n",
            "        // Prevents `serde_core::ser::Serialize` appearing in the error message\n",
            "        // in projects with no direct dependency on serde_core.\n",
            "        message = \"the trait bound `{Self}: serde::Serialize` is not satisfied\",\n",
            "        note = \"for local types consider adding `#[derive(serde::Serialize)]` to your `{Self}` type\",\n",
            "        note = \"for types from other crates check whether the crate offers a `serde` feature flag\",\n",
            "    )\n",
            ")]\n",
            "pub trait Serialize {\n",
            "    fn serialize<S>(&self, s: S);\n",
            "}\n",
        );
        let result = extract_item_from_source(src, "Serialize");
        assert!(result.is_some(), "should find Serialize");
        let (kind, _sig, doc_text, _examples) = result.unwrap();
        assert_eq!(kind, "trait");
        assert!(
            !doc_text.is_empty(),
            "doc_text should not be empty, got: {:?}",
            doc_text
        );
        assert!(
            doc_text.contains("serialized"),
            "expected 'serialized' in: {:?}",
            doc_text
        );
    }

    #[test]
    fn test_local_extractor_serde_serialize() {
        // This test requires serde to be a dependency in the workspace
        // (it is, via normalize-ecosystems deps chain)
        use std::path::Path;
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let extractor = CargoLocalDocsExtractor::new(project_root);
        let result = extractor.extract_docs("serde", "serde::Serialize", None);
        match result {
            Ok(doc) => {
                assert_eq!(doc.name, "Serialize");
                assert_eq!(doc.package, "serde");
                assert_eq!(doc.kind, "trait");
                assert!(
                    !doc.doc_text.is_empty(),
                    "doc_text should not be empty, got markdown:\n{}",
                    doc.to_markdown()
                );
                println!("Local doc extraction succeeded:\n{}", doc.to_markdown());
            }
            Err(e) => panic!("local extraction failed: {:?}", e),
        }
    }
}
