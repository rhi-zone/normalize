//! End-to-end fixture tests for symbol/import extraction.
//!
//! Each fixture is a tiny, runnable multi-file project under:
//!   `tests/fixtures/<lang>/<case>/project/`
//!
//! The runner:
//!   1. Extracts symbols + imports from every source file in `project/`
//!   2. Compares against `expected/symbols.json` and `expected/imports.json`
//!   3. Runs the project (if runtime available) and compares stdout against
//!      `expected/stdout.txt`
//!
//! Set `UPDATE_FIXTURES=1` to regenerate all `expected/` files from actual output.

use normalize_facts::SymbolParser;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Fixture discovery
// ---------------------------------------------------------------------------

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Returns all fixture case dirs — each must contain a `project/` subdir.
fn find_fixture_cases() -> Vec<PathBuf> {
    let root = fixtures_root();
    if !root.exists() {
        return Vec::new();
    }
    let mut cases = Vec::new();
    for lang_entry in sorted_entries(&root) {
        if !lang_entry.is_dir() {
            continue;
        }
        for case_entry in sorted_entries(&lang_entry) {
            if case_entry.is_dir() && case_entry.join("project").is_dir() {
                cases.push(case_entry);
            }
        }
    }
    cases
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    entries
}

/// Derive lang from path: `tests/fixtures/python/add_numbers` → `python`
fn lang_from_case(case_dir: &Path) -> String {
    let root = fixtures_root();
    let rel = case_dir.strip_prefix(&root).unwrap();
    rel.components()
        .next()
        .unwrap()
        .as_os_str()
        .to_str()
        .unwrap()
        .to_string()
}

// ---------------------------------------------------------------------------
// Symbol + import extraction
// ---------------------------------------------------------------------------

/// Collect all source files under a directory (non-recursive would miss subdirs).
fn collect_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files);
    files.sort();
    files
}

/// Directories to skip — build artefacts, caches, hidden dirs.
const SKIP_DIRS: &[&str] = &[
    "target",       // Rust
    "node_modules", // JavaScript / TypeScript
    "__pycache__",  // Python
    ".git",
    "dist",
    "build",
    ".cache",
];

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !SKIP_DIRS.contains(&name) {
                collect_files_recursive(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

fn extract_symbols_json(project_dir: &Path) -> Value {
    let parser = SymbolParser::new();
    let mut rows: Vec<Value> = Vec::new();

    for path in collect_source_files(project_dir) {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let symbols = parser.parse_file(&path, &content).unwrap_or_default();
        let rel = path.strip_prefix(project_dir).unwrap().to_slash_lossy();
        for sym in symbols {
            rows.push(json!({
                "file": rel,
                "name": sym.name,
                "kind": format!("{:?}", sym.kind),
                "start_line": sym.start_line,
                "parent": sym.parent,
            }));
        }
    }
    Value::Array(rows)
}

/// Find the first recognized manifest file in a project dir and parse it.
fn extract_manifest_json(project_dir: &Path) -> Option<Value> {
    const MANIFEST_FILENAMES: &[&str] = &[
        "Cargo.toml",
        "go.mod",
        "package.json",
        "requirements.txt",
        "pyproject.toml",
    ];
    for filename in MANIFEST_FILENAMES {
        let path = project_dir.join(filename);
        if path.is_file()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Some(manifest) = normalize_manifest::parse_manifest(filename, &content)
        {
            return Some(serde_json::to_value(&manifest).unwrap());
        }
    }
    None
}

fn extract_imports_json(project_dir: &Path) -> Value {
    let parser = SymbolParser::new();
    let mut rows: Vec<Value> = Vec::new();

    for path in collect_source_files(project_dir) {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let imports = parser.parse_imports(&path, &content);
        let rel = path.strip_prefix(project_dir).unwrap().to_slash_lossy();
        for imp in imports {
            rows.push(json!({
                "file": rel,
                "module": imp.module,
                "name": imp.name,
                "alias": imp.alias,
                "line": imp.line,
            }));
        }
    }
    Value::Array(rows)
}

// ---------------------------------------------------------------------------
// Project execution
// ---------------------------------------------------------------------------

fn runtime_available(lang: &str) -> bool {
    let (cmd, args): (&str, &[&str]) = match lang {
        "python" => ("python3", &["--version"]),
        "javascript" => ("node", &["--version"]),
        "go" => ("go", &["version"]),
        "rust" => ("cargo", &["--version"]),
        "typescript" => ("npx", &["ts-node", "--version"]),
        _ => return false,
    };
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run the project and return stdout, or None if runtime unavailable / build fails.
fn run_project(project_dir: &Path, lang: &str) -> Option<String> {
    if !runtime_available(lang) {
        return None;
    }
    let output = match lang {
        "python" => Command::new("python3")
            .arg("main.py")
            .current_dir(project_dir)
            .output()
            .ok()?,
        "javascript" => Command::new("node")
            .arg("index.js")
            .current_dir(project_dir)
            .output()
            .ok()?,
        "go" => Command::new("go")
            .args(["run", "."])
            .current_dir(project_dir)
            .output()
            .ok()?,
        "rust" => Command::new("cargo")
            .args(["run", "--quiet"])
            .current_dir(project_dir)
            .output()
            .ok()?,
        "typescript" => Command::new("npx")
            .args(["ts-node", "index.ts"])
            .current_dir(project_dir)
            .output()
            .ok()?,
        _ => return None,
    };
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("WARN: {lang} project run failed:\n{stderr}");
        None
    }
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

fn update_mode() -> bool {
    std::env::var("UPDATE_FIXTURES").is_ok()
}

fn assert_json_eq(actual: &Value, expected_path: &Path, label: &str) {
    let expected_str = std::fs::read_to_string(expected_path)
        .unwrap_or_else(|_| panic!("{label}: expected file not found: {expected_path:?}"));
    let expected: Value = serde_json::from_str(&expected_str)
        .unwrap_or_else(|e| panic!("{label}: invalid JSON in {expected_path:?}: {e}"));

    if actual != &expected {
        eprintln!("\n=== FAIL: {label} ===");
        eprintln!("--- expected ---");
        eprintln!("{}", serde_json::to_string_pretty(&expected).unwrap());
        eprintln!("--- actual ---");
        eprintln!("{}", serde_json::to_string_pretty(actual).unwrap());
        eprintln!("=================\n");
        panic!("{label}: output mismatch. Run UPDATE_FIXTURES=1 cargo test to regenerate.");
    }
}

fn write_or_compare_json(actual: &Value, expected_path: &Path, label: &str) {
    if update_mode() {
        if let Some(parent) = expected_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let json = serde_json::to_string_pretty(actual).unwrap() + "\n";
        std::fs::write(expected_path, json)
            .unwrap_or_else(|e| panic!("Failed to write {expected_path:?}: {e}"));
        eprintln!("UPDATED: {expected_path:?}");
    } else {
        assert_json_eq(actual, expected_path, label);
    }
}

fn write_or_compare_text(actual: &str, expected_path: &Path, label: &str) {
    if update_mode() {
        if let Some(parent) = expected_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(expected_path, actual)
            .unwrap_or_else(|e| panic!("Failed to write {expected_path:?}: {e}"));
        eprintln!("UPDATED: {expected_path:?}");
    } else {
        let expected = std::fs::read_to_string(expected_path)
            .unwrap_or_else(|_| panic!("{label}: expected file not found: {expected_path:?}"));
        if actual != expected {
            eprintln!("\n=== FAIL: {label} ===");
            eprintln!("--- expected ---\n{expected}--- actual ---\n{actual}=================\n");
            panic!("{label}: stdout mismatch. Run UPDATE_FIXTURES=1 cargo test to regenerate.");
        }
    }
}

// ---------------------------------------------------------------------------
// Main test
// ---------------------------------------------------------------------------

#[test]
fn extract_fixtures() {
    let cases = find_fixture_cases();

    if !update_mode() {
        assert!(
            !cases.is_empty(),
            "No fixture cases found under tests/fixtures/. \
             Each case needs a project/ subdirectory."
        );
    }

    let mut passed = 0;
    let mut skipped_exec = 0;

    for case_dir in &cases {
        let lang = lang_from_case(case_dir);
        let case_name = case_dir.file_name().unwrap().to_str().unwrap();
        let project_dir = case_dir.join("project");
        let expected_dir = case_dir.join("expected");

        let label = format!("{lang}/{case_name}");
        eprintln!("Testing {label}...");

        // --- symbols ---
        let symbols_expected = expected_dir.join("symbols.json");
        if symbols_expected.exists() || update_mode() {
            let actual = extract_symbols_json(&project_dir);
            write_or_compare_json(&actual, &symbols_expected, &format!("{label} symbols"));
        }

        // --- imports ---
        let imports_expected = expected_dir.join("imports.json");
        if imports_expected.exists() || update_mode() {
            let actual = extract_imports_json(&project_dir);
            write_or_compare_json(&actual, &imports_expected, &format!("{label} imports"));
        }

        // --- manifest ---
        let manifest_expected = expected_dir.join("manifest.json");
        if (manifest_expected.exists() || update_mode())
            && let Some(manifest) = extract_manifest_json(&project_dir)
        {
            write_or_compare_json(&manifest, &manifest_expected, &format!("{label} manifest"));
        }

        // --- execution ---
        let stdout_expected = expected_dir.join("stdout.txt");
        if stdout_expected.exists() || update_mode() {
            if let Some(stdout) = run_project(&project_dir, &lang) {
                write_or_compare_text(&stdout, &stdout_expected, &format!("{label} stdout"));
            } else {
                eprintln!("  SKIP execution: {lang} runtime not available");
                skipped_exec += 1;
            }
        }

        passed += 1;
    }

    eprintln!(
        "\nextract_fixtures: {passed} cases, {skipped_exec} execution tests skipped (missing runtime)"
    );
}

// ---------------------------------------------------------------------------
// Path helper
// ---------------------------------------------------------------------------

trait ToSlashLossy {
    fn to_slash_lossy(&self) -> String;
}

impl ToSlashLossy for Path {
    fn to_slash_lossy(&self) -> String {
        self.to_string_lossy().replace('\\', "/")
    }
}
