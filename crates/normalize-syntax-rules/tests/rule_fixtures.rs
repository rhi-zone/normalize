//! Fixture-based tests for all builtin syntax rules.
//!
//! Structure:
//!   tests/fixtures/<lang>/<rule-name>/match.<ext>          — must produce ≥1 findings
//!   tests/fixtures/<lang>/<rule-name>/no_match.<ext>       — must produce 0 findings
//!   tests/fixtures/<lang>/<rule-name>/fix.<ext>            — input for auto-fix test
//!   tests/fixtures/<lang>/<rule-name>/fix.expected.<ext>   — expected output after fix
//!
//! Top-level rules (no namespace): tests/fixtures/<rule-name>/match.<ext>
//!
//! The rule ID is derived from the fixture directory path relative to `tests/fixtures/`,
//! joining path components with `/`. e.g. `fixtures/rust/static-mut/` → `rust/static-mut`.

use normalize_languages::GrammarLoader;
use normalize_syntax_rules::{DebugFlags, apply_fixes, load_all_rules, run_rules};
use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Recursively find fixture directories (those containing `match.*`, `no_match.*`, or `fix.*` files).
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
                    (s.starts_with("match.") || s.starts_with("no_match.") || s.starts_with("fix."))
                        && e.path().is_file()
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

/// Find a file in `dir` whose name starts with `prefix.` but not `prefix.expected.`.
/// Returns `(path, extension)` or None if not found.
fn find_fixture_file(dir: &Path, prefix: &str) -> Option<(PathBuf, String)> {
    let expected_prefix = format!("{prefix}.expected.");
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let name = e.file_name();
        let s = name.to_string_lossy().into_owned();
        if s.starts_with(&format!("{prefix}."))
            && !s.starts_with(&expected_prefix)
            && e.path().is_file()
        {
            let ext = s[prefix.len() + 1..].to_string();
            Some((e.path(), ext))
        } else {
            None
        }
    })
}

/// Find the `fix.expected.<ext>` file for a given extension.
fn find_expected_file(dir: &Path, ext: &str) -> Option<PathBuf> {
    let name = format!("fix.expected.{ext}");
    let path = dir.join(&name);
    path.is_file().then_some(path)
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
        // Must iterate ALL rules (not use `any()` which short-circuits).
        let mut found = false;
        for r in rules.iter_mut() {
            if r.id == rule_id {
                r.enabled = true;
                found = true;
            } else {
                r.enabled = false;
            }
        }
        if !found {
            failures.push(format!(
                "MISSING RULE: `{rule_id}` — no builtin rule found for this fixture directory"
            ));
            continue;
        }

        // Run against the entire fixture directory.
        let findings = run_rules(
            &rules,
            fixture_dir,
            fixture_dir,
            &loader,
            None,
            None,
            None,
            &debug,
        );

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

        // --- Fix fixture test ---
        // If fix.<ext> exists, apply fixes and compare to fix.expected.<ext>.
        if let Some((fix_src, ext)) = find_fixture_file(fixture_dir, "fix") {
            match run_fix_fixture(
                fixture_dir,
                &fix_src,
                &ext,
                &rule_id,
                &rules,
                &loader,
                &debug,
            ) {
                Ok(()) => {}
                Err(msg) => failures.push(msg),
            }
        }
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

/// Run the fix fixture test for one rule:
/// 1. Copy `fix.<ext>` to a temp directory.
/// 2. Run the rule against the temp dir.
/// 3. Apply fixes (loop until stable).
/// 4. Compare the result to `fix.expected.<ext>`.
fn run_fix_fixture(
    fixture_dir: &Path,
    fix_src: &Path,
    ext: &str,
    rule_id: &str,
    rules: &[normalize_syntax_rules::Rule],
    loader: &GrammarLoader,
    debug: &DebugFlags,
) -> Result<(), String> {
    let expected_path = match find_expected_file(fixture_dir, ext) {
        Some(p) => p,
        None => {
            return Err(format!(
                "`{rule_id}`: fix.{ext} exists but fix.expected.{ext} is missing"
            ));
        }
    };

    let expected = std::fs::read_to_string(&expected_path)
        .map_err(|e| format!("`{rule_id}`: failed to read fix.expected.{ext}: {e}"))?;

    let input = std::fs::read_to_string(fix_src)
        .map_err(|e| format!("`{rule_id}`: failed to read fix.{ext}: {e}"))?;

    // Work in a temp dir inside the fixture dir so Cargo.toml walk-up works
    // for `requires` checks (e.g., rust.edition).
    let tmp = tempfile::tempdir_in(fixture_dir)
        .map_err(|e| format!("`{rule_id}`: failed to create tempdir: {e}"))?;
    let tmp_file = tmp.path().join(format!("fix.{ext}"));
    std::fs::write(&tmp_file, &input)
        .map_err(|e| format!("`{rule_id}`: failed to write temp fix file: {e}"))?;

    // Apply fixes in a loop until stable (handles multi-pass nested fixes).
    const MAX_PASSES: usize = 10;
    for pass in 0..MAX_PASSES {
        let findings = run_rules(
            rules,
            tmp.path(),
            tmp.path(),
            loader,
            None,
            None,
            None,
            debug,
        );
        let fixable: Vec<_> = findings.into_iter().filter(|f| f.fix.is_some()).collect();
        if fixable.is_empty() {
            break;
        }
        apply_fixes(&fixable)
            .map_err(|e| format!("`{rule_id}`: apply_fixes pass {pass} failed: {e}"))?;
    }

    let actual = std::fs::read_to_string(&tmp_file)
        .map_err(|e| format!("`{rule_id}`: failed to read fixed output: {e}"))?;

    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "`{rule_id}`: fix output mismatch\n--- expected (fix.expected.{ext}) ---\n{expected}\n--- actual ---\n{actual}"
        ))
    }
}
