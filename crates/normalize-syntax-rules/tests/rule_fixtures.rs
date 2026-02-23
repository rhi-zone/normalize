//! Fixture-based tests for all builtin syntax rules.
//!
//! Structure:
//!   tests/fixtures/<lang>/<rule-name>/match.<ext>     — must produce ≥1 findings
//!   tests/fixtures/<lang>/<rule-name>/no_match.<ext>  — must produce 0 findings
//!
//! Top-level rules (no namespace): tests/fixtures/<rule-name>/match.<ext>
//!
//! The rule ID is derived from the fixture directory path relative to `tests/fixtures/`,
//! joining path components with `/`. e.g. `fixtures/rust/static-mut/` → `rust/static-mut`.

use normalize_languages::GrammarLoader;
use normalize_syntax_rules::{DebugFlags, load_all_rules, run_rules};
use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Recursively find fixture directories (those containing `match.*` or `no_match.*` files).
fn find_fixture_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let has_fixtures = std::fs::read_dir(&path)
            .map(|es| {
                es.flatten().any(|e| {
                    let name = e.file_name();
                    let s = name.to_string_lossy();
                    (s.starts_with("match.") || s.starts_with("no_match.")) && e.path().is_file()
                })
            })
            .unwrap_or(false);
        if has_fixtures {
            result.push(path);
        } else {
            result.extend(find_fixture_dirs(&path));
        }
    }
    result
}

/// Derive rule ID from a fixture directory path relative to the fixtures root.
/// `fixtures/rust/static-mut/` → `rust/static-mut`
/// `fixtures/no-todo-comment/` → `no-todo-comment`
fn derive_rule_id(fixture_dir: &Path, fixtures_root: &Path) -> String {
    fixture_dir
        .strip_prefix(fixtures_root)
        .expect("fixture dir must be under fixtures root")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn test_rule_fixtures() {
    let fixtures_root = fixtures_dir();
    let loader = GrammarLoader::new();
    let debug = DebugFlags { timing: false };

    let mut failures: Vec<String> = Vec::new();
    let mut tested = 0;

    let fixture_dirs = {
        let mut dirs = find_fixture_dirs(&fixtures_root);
        dirs.sort(); // deterministic order
        dirs
    };

    for fixture_dir in &fixture_dirs {
        let rule_id = derive_rule_id(fixture_dir, &fixtures_root);

        // Load builtins (fixture dirs have no .normalize/rules/, so only builtins load).
        let mut rules = load_all_rules(fixture_dir, &Default::default());

        // Enable the rule under test, disable everything else.
        let found = rules.iter_mut().any(|r| {
            if r.id == rule_id {
                r.enabled = true;
                true
            } else {
                r.enabled = false;
                false
            }
        });
        if !found {
            failures.push(format!(
                "MISSING RULE: `{rule_id}` — no builtin rule found for this fixture directory"
            ));
            continue;
        }

        // Run against the entire fixture directory.
        let findings = run_rules(&rules, fixture_dir, &loader, None, None, None, &debug);

        // Partition findings by fixture file type.
        let match_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.file.file_stem().map(|s| s == "match").unwrap_or(false))
            .collect();
        let no_match_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.file.file_stem().map(|s| s == "no_match").unwrap_or(false))
            .collect();

        // Check match.* file if it exists.
        let match_file_exists = std::fs::read_dir(fixture_dir)
            .map(|es| {
                es.flatten()
                    .any(|e| e.file_name().to_string_lossy().starts_with("match."))
            })
            .unwrap_or(false);
        if match_file_exists && match_findings.is_empty() {
            failures.push(format!(
                "`{rule_id}`: match.* produced no findings (expected ≥1)"
            ));
        }

        // Check no_match.* file — must produce zero findings.
        if !no_match_findings.is_empty() {
            let details: Vec<_> = no_match_findings
                .iter()
                .map(|f| {
                    format!(
                        "    {}:{}: {}",
                        f.file.display(),
                        f.start_line,
                        f.matched_text
                    )
                })
                .collect();
            failures.push(format!(
                "`{rule_id}`: no_match.* produced {} unexpected finding(s):\n{}",
                no_match_findings.len(),
                details.join("\n")
            ));
        }

        tested += 1;
    }

    if !failures.is_empty() {
        panic!(
            "{} rule fixture failure(s) (out of {} tested):\n\n{}",
            failures.len(),
            tested,
            failures.join("\n\n")
        );
    }

    // Sanity check: at least some rules were tested.
    assert!(
        tested >= 10,
        "expected at least 10 fixture tests, only found {tested} — are the fixture files missing?"
    );
    println!("Tested {tested} rule fixtures.");
}
