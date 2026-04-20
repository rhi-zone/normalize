//! Directory context: hierarchical context files.
//!
//! Resolves contextual text from `.normalize/context/` directories walked
//! hierarchically (project → parent → ... → global `~/.normalize/context/`).
//! Source files are Markdown with optional YAML frontmatter. Frontmatter is
//! matched against caller-provided context to filter which blocks are returned.
//!
//! Domain logic lives in `normalize-context`. This module contains:
//! - CLI input helpers: `parse_match_pairs`, `read_stdin_context`
//! - Legacy `.context.md`/`CONTEXT.md` walk (`collect_context_files`, `get_merged_context`)
//!
//! `OutputFormatter` impls for report structs live in `normalize-context` behind the `cli` feature.

use std::io::Read as _;
use std::path::{Path, PathBuf};

// Re-export domain types used by service/mod.rs
pub use normalize_context::{
    CallerContext, ContextBlock, ContextListReport, ContextReport, collect_new_context_files,
    resolve_context, yaml_to_json,
};

// ---------------------------------------------------------------------------
// Legacy support: the old `.context.md` / `CONTEXT.md` walk
//
// Kept because `service/view.rs` `--dir-context` still uses it.
// ---------------------------------------------------------------------------

/// Legacy context file names (old system).
const LEGACY_CONTEXT_FILES: &[&str] = &[".context.md", "CONTEXT.md"];

/// Collect legacy `.context.md` / `CONTEXT.md` files from target up to root.
///
/// Returns files in **target→root order** (most specific first).
/// `max_depth` limits how many ancestor levels above target to include:
/// `None` means unlimited.
pub fn collect_context_files(
    root: &Path,
    target_dir: &Path,
    max_depth: Option<usize>,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut current = target_dir.to_path_buf();
    let mut depth = 0usize;

    loop {
        if !current.starts_with(root) {
            break;
        }
        if let Some(max) = max_depth
            && depth > max
        {
            break;
        }
        for name in LEGACY_CONTEXT_FILES {
            let path = current.join(name);
            if path.exists() {
                files.push(path);
                break;
            }
        }
        if current == root {
            break;
        }
        match current.parent() {
            Some(p) => {
                current = p.to_path_buf();
                depth += 1;
            }
            None => break,
        }
    }

    files
}

/// Get merged context content for a path (legacy system).
/// Used by `view --dir-context`.
pub fn get_merged_context(root: &Path, target: &Path, max_depth: Option<usize>) -> Option<String> {
    let target_dir = if target.is_file() {
        target.parent().unwrap_or(root).to_path_buf()
    } else if target.is_dir() {
        target.to_path_buf()
    } else {
        let mut dir = target.to_path_buf();
        while !dir.exists() {
            match dir.parent() {
                Some(p) => dir = p.to_path_buf(),
                None => return None,
            }
        }
        dir
    };

    let root = root.canonicalize().ok()?;
    let target_dir = target_dir.canonicalize().ok()?;

    let files_target_to_root = collect_context_files(&root, &target_dir, max_depth);
    if files_target_to_root.is_empty() {
        return None;
    }

    let mut content = String::new();
    for (i, file) in files_target_to_root.iter().rev().enumerate() {
        if i > 0 {
            content.push_str("\n\n");
        }
        if let Ok(text) = std::fs::read_to_string(file) {
            content.push_str(&text);
        }
    }

    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

// ---------------------------------------------------------------------------
// CLI input helpers
// ---------------------------------------------------------------------------

/// Parse `--match KEY=VALUE` pairs into a [`CallerContext`].
pub fn parse_match_pairs(pairs: &[String]) -> Result<CallerContext, String> {
    let mut map = CallerContext::new();
    for pair in pairs {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        } else {
            return Err(format!("--match argument must be KEY=VALUE, got: {pair:?}"));
        }
    }
    Ok(map)
}

/// Read a structured file and merge into caller context under `prefix`.
///
/// Dispatches on file extension:
/// - `.json`  → JSON
/// - `.toml`  → TOML
/// - `.yaml` / `.yml` → YAML
/// - `.ini` / `.kdl` / `.hcl` → not yet implemented
///
/// The parsed value is flattened into dot-path key-value pairs under `prefix`.
pub fn read_file_context(prefix: &str, path: &str) -> Result<CallerContext, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file {path:?}: {e}"))?;

    let value: serde_json::Value = match ext.as_str() {
        "json" => serde_json::from_str(&content)
            .map_err(|e| format!("{path:?} is not valid JSON: {e}"))?,
        "toml" => {
            let toml_value: toml::Value =
                toml::from_str(&content).map_err(|e| format!("{path:?} is not valid TOML: {e}"))?;
            serde_json::to_value(toml_value)
                .map_err(|e| format!("Failed to convert TOML to JSON value: {e}"))?
        }
        "yaml" | "yml" => serde_yaml::from_str(&content)
            .map_err(|e| format!("{path:?} is not valid YAML: {e}"))?,
        "ini" | "kdl" | "hcl" => {
            return Err(format!(
                "File format {ext:?} is not yet implemented for --file"
            ));
        }
        other => {
            return Err(format!(
                "Unrecognized file extension {other:?} for --file; \
                 supported: .json, .toml, .yaml, .yml"
            ));
        }
    };

    let nested = serde_json::json!({ prefix: value });
    let mut map = CallerContext::new();
    flatten_json(&nested, "", &mut map);
    Ok(map)
}

/// Read JSON from stdin and merge into caller context, optionally under `prefix`.
pub fn read_stdin_context(prefix: Option<&str>) -> Result<CallerContext, String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("Failed to read stdin: {e}"))?;

    if buf.trim().is_empty() {
        return Ok(CallerContext::new());
    }

    let value: serde_json::Value =
        serde_json::from_str(&buf).map_err(|e| format!("stdin is not valid JSON: {e}"))?;

    let nested: serde_json::Value = if let Some(pfx) = prefix {
        serde_json::json!({ pfx: value })
    } else {
        value
    };

    let mut map = CallerContext::new();
    flatten_json(&nested, "", &mut map);
    Ok(map)
}

/// Flatten a JSON object into dot-path string pairs.
fn flatten_json(value: &serde_json::Value, prefix: &str, out: &mut CallerContext) {
    match value {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(v, &key, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let key = if prefix.is_empty() {
                    i.to_string()
                } else {
                    format!("{prefix}.{i}")
                };
                flatten_json(v, &key, out);
            }
        }
        other => {
            out.insert(prefix.to_string(), json_scalar_to_string(other));
        }
    }
}

fn json_scalar_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // --- Legacy collect_context_files tests (unchanged behavior) ---

    #[test]
    fn test_collect_single_context_file() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();
        let files = collect_context_files(root, root, None);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("CONTEXT.md"));
    }

    #[test]
    fn test_collect_hierarchical_context_target_to_root_order() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/.context.md"), "Src context").unwrap();
        let files = collect_context_files(root, &root.join("src"), None);
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with(".context.md"));
        assert!(files[1].ends_with("CONTEXT.md"));
    }

    #[test]
    fn test_collect_max_depth_zero() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/.context.md"), "Src context").unwrap();
        let files = collect_context_files(root, &root.join("src"), Some(0));
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".context.md"));
    }

    #[test]
    fn test_collect_max_depth_one() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/.context.md"), "A context").unwrap();
        fs::write(root.join("a/b/.context.md"), "B context").unwrap();
        let files = collect_context_files(root, &root.join("a/b"), Some(1));
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("b/.context.md"));
        assert!(files[1].ends_with("a/.context.md"));
    }

    #[test]
    fn test_dotfile_takes_priority() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CONTEXT.md"), "Uppercase").unwrap();
        fs::write(root.join(".context.md"), "Dotfile").unwrap();
        let files = collect_context_files(root, root, None);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".context.md"));
    }

    #[test]
    fn test_get_merged_context_root_to_target_order() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("CONTEXT.md"), "Root").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/.context.md"), "Sub").unwrap();
        let content = get_merged_context(root, &root.join("sub/file.rs"), None).unwrap();
        let root_pos = content.find("Root").unwrap();
        let sub_pos = content.find("Sub").unwrap();
        assert!(
            root_pos < sub_pos,
            "root→target order: Root should come before Sub"
        );
    }

    #[test]
    fn test_no_context_files() {
        let tmp = tempdir().unwrap();
        let files = collect_context_files(tmp.path(), tmp.path(), None);
        assert!(files.is_empty());
    }

    // --- CLI input helper tests ---

    #[test]
    fn test_parse_match_pairs() {
        let pairs = vec!["hook=UserPromptSubmit".to_string(), "lang=rust".to_string()];
        let ctx = parse_match_pairs(&pairs).unwrap();
        assert_eq!(ctx.get("hook").unwrap(), "UserPromptSubmit");
        assert_eq!(ctx.get("lang").unwrap(), "rust");
    }

    #[test]
    fn test_flatten_json() {
        let json: serde_json::Value =
            serde_json::json!({"claudecode": {"hook": "UserPromptSubmit"}});
        let mut map = CallerContext::new();
        flatten_json(&json, "", &mut map);
        assert_eq!(map.get("claudecode.hook").unwrap(), "UserPromptSubmit");
    }
}
