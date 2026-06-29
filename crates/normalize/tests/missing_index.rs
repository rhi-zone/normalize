//! Integration tests for the missing-index guard and the `rules show` native
//! lookup.
//!
//! These exercise the hard-constraint fix "never silently return empty
//! results": import-graph commands must exit non-zero (and emit a structured
//! JSON error under `--json`) when the index has no import data, rather than
//! returning a zeroed report with exit 0. They also cover the `rules show`
//! native-rule resolution fix.

use assert_cmd::{Command, cargo_bin_cmd};
use tempfile::TempDir;

fn normalize() -> Command {
    cargo_bin_cmd!("normalize")
}

/// A project with no source files has an empty `imports` table, so the
/// import-graph guard must fire. Run inside an isolated daemon config dir so the
/// auto-start path can't touch a real daemon socket.
fn empty_project() -> TempDir {
    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("notes.txt"), "not source code\n").unwrap();
    // Isolated daemon config dir (subdir of the project, cleaned up with it) so
    // the auto-start path can't touch a real daemon socket.
    std::fs::create_dir_all(project.path().join("daemon")).unwrap();
    project
}

#[test]
fn view_graph_json_emits_structured_error_on_empty_import_graph() {
    let project = empty_project();
    let out = normalize()
        .current_dir(project.path())
        .env("NORMALIZE_DAEMON_CONFIG_DIR", project.path().join("daemon"))
        .args(["view", "graph", "--json"])
        .output()
        .expect("run view graph --json");

    assert!(
        !out.status.success(),
        "view graph --json must exit non-zero on empty import graph; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"error\""),
        "expected a JSON error object on stdout under --json, got: {stdout}"
    );
}

#[test]
fn rank_imports_exits_nonzero_with_rebuild_hint_on_empty_import_graph() {
    let project = empty_project();
    let out = normalize()
        .current_dir(project.path())
        .env("NORMALIZE_DAEMON_CONFIG_DIR", project.path().join("daemon"))
        .args(["rank", "imports"])
        .output()
        .expect("run rank imports");

    assert!(
        !out.status.success(),
        "rank imports must exit non-zero on empty import graph"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("structure rebuild"),
        "expected an actionable `structure rebuild` hint, got: {combined}"
    );
}

#[test]
fn rules_show_resolves_native_rule() {
    // `stale-summary` is a native rule: it appears in `rules list` and must be
    // resolvable by `rules show` (previously reported "Rule not found").
    let daemon_dir = TempDir::new().unwrap();
    let out = normalize()
        .env("NORMALIZE_DAEMON_CONFIG_DIR", daemon_dir.path())
        .args(["rules", "show", "stale-summary"])
        .output()
        .expect("run rules show stale-summary");

    assert!(
        out.status.success(),
        "rules show stale-summary must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("stale-summary") && stdout.contains("native"),
        "expected native rule detail for stale-summary, got: {stdout}"
    );
}

#[test]
fn rules_show_json_native_rule_has_type_native() {
    let daemon_dir = TempDir::new().unwrap();
    let out = normalize()
        .env("NORMALIZE_DAEMON_CONFIG_DIR", daemon_dir.path())
        .args(["rules", "show", "stale-summary", "--json"])
        .output()
        .expect("run rules show stale-summary --json");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"rule_type\"") && stdout.contains("native"),
        "expected JSON with rule_type=native, got: {stdout}"
    );
}
