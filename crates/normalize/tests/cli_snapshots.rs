//! CLI snapshot tests - verify --help output doesn't change unexpectedly.
//!
//! These tests ensure CLI breaking changes are detected during review.
//! Run `cargo insta review` to update snapshots after intentional changes.

use assert_cmd::{Command, cargo_bin_cmd};

fn normalize() -> Command {
    cargo_bin_cmd!("normalize")
}

fn snapshot_help(args: &[&str]) -> String {
    let mut cmd = normalize();
    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg("--help");

    let output = cmd.output().expect("failed to execute normalize");
    String::from_utf8_lossy(&output.stdout).to_string()
}

// Root command
#[test]
fn test_help_root() {
    insta::assert_snapshot!(snapshot_help(&[]));
}

// Top-level commands
#[test]
fn test_help_view() {
    insta::assert_snapshot!(snapshot_help(&["view"]));
}

#[test]
fn test_help_edit() {
    insta::assert_snapshot!(snapshot_help(&["edit"]));
}

#[test]
fn test_help_history() {
    insta::assert_snapshot!(snapshot_help(&["history"]));
}

#[test]
fn test_help_structure() {
    insta::assert_snapshot!(snapshot_help(&["structure"]));
}

#[test]
fn test_help_init() {
    insta::assert_snapshot!(snapshot_help(&["init"]));
}

#[test]
fn test_help_daemon() {
    insta::assert_snapshot!(snapshot_help(&["daemon"]));
}

#[test]
fn test_help_update() {
    insta::assert_snapshot!(snapshot_help(&["update"]));
}

#[test]
fn test_help_grammars() {
    insta::assert_snapshot!(snapshot_help(&["grammars"]));
}

#[test]
fn test_help_analyze() {
    insta::assert_snapshot!(snapshot_help(&["analyze"]));
}

#[test]
fn test_help_aliases() {
    insta::assert_snapshot!(snapshot_help(&["aliases"]));
}

#[test]
fn test_help_find_references() {
    insta::assert_snapshot!(snapshot_help(&["find-references"]));
}

#[test]
fn test_help_context() {
    insta::assert_snapshot!(snapshot_help(&["context"]));
}

#[test]
fn test_help_grep() {
    insta::assert_snapshot!(snapshot_help(&["grep"]));
}

#[test]
fn test_help_sessions() {
    insta::assert_snapshot!(snapshot_help(&["sessions"]));
}

#[test]
fn test_help_package() {
    insta::assert_snapshot!(snapshot_help(&["package"]));
}

#[test]
fn test_help_tools() {
    insta::assert_snapshot!(snapshot_help(&["tools"]));
}

#[test]
fn test_help_serve() {
    insta::assert_snapshot!(snapshot_help(&["serve"]));
}

#[test]
fn test_help_generate() {
    insta::assert_snapshot!(snapshot_help(&["generate"]));
}

#[test]
fn test_help_rules() {
    insta::assert_snapshot!(snapshot_help(&["rules"]));
}

// edit subcommands
#[test]
fn test_help_edit_delete() {
    insta::assert_snapshot!(snapshot_help(&["edit", "delete"]));
}

#[test]
fn test_help_edit_replace() {
    insta::assert_snapshot!(snapshot_help(&["edit", "replace"]));
}

#[test]
fn test_help_edit_swap() {
    insta::assert_snapshot!(snapshot_help(&["edit", "swap"]));
}

#[test]
fn test_help_edit_insert() {
    insta::assert_snapshot!(snapshot_help(&["edit", "insert"]));
}

// structure subcommands
#[test]
fn test_help_structure_rebuild() {
    insta::assert_snapshot!(snapshot_help(&["structure", "rebuild"]));
}

#[test]
fn test_help_structure_stats() {
    insta::assert_snapshot!(snapshot_help(&["structure", "stats"]));
}

#[test]
fn test_help_structure_files() {
    insta::assert_snapshot!(snapshot_help(&["structure", "files"]));
}

#[test]
fn test_help_structure_packages() {
    insta::assert_snapshot!(snapshot_help(&["structure", "packages"]));
}

// daemon subcommands
#[test]
fn test_help_daemon_status() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "status"]));
}

#[test]
fn test_help_daemon_stop() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "stop"]));
}

#[test]
fn test_help_daemon_start() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "start"]));
}

#[test]
fn test_help_daemon_run() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "run"]));
}

#[test]
fn test_help_daemon_add() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "add"]));
}

#[test]
fn test_help_daemon_remove() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "remove"]));
}

#[test]
fn test_help_daemon_list() {
    insta::assert_snapshot!(snapshot_help(&["daemon", "list"]));
}

// grammars subcommands
#[test]
fn test_help_grammars_list() {
    insta::assert_snapshot!(snapshot_help(&["grammars", "list"]));
}

#[test]
fn test_help_grammars_install() {
    insta::assert_snapshot!(snapshot_help(&["grammars", "install"]));
}

#[test]
fn test_help_grammars_paths() {
    insta::assert_snapshot!(snapshot_help(&["grammars", "paths"]));
}

// analyze subcommands
#[test]
fn test_help_analyze_health() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "health"]));
}

#[test]
fn test_help_analyze_complexity() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "complexity"]));
}

#[test]
fn test_help_analyze_length() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "length"]));
}

#[test]
fn test_help_analyze_trend_metric() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "trend-metric"]));
}

#[test]
fn test_help_analyze_security() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "security"]));
}

#[test]
fn test_help_analyze_docs() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "docs"]));
}

#[test]
fn test_help_analyze_files() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "files"]));
}

#[test]
fn test_help_analyze_trace() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "trace"]));
}

#[test]
fn test_help_analyze_ownership() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "ownership"]));
}

#[test]
fn test_help_analyze_repo_coupling() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "repo-coupling"]));
}

#[test]
fn test_help_analyze_duplicate_types() {
    insta::assert_snapshot!(snapshot_help(&["analyze", "duplicate-types"]));
}

// sessions subcommands
#[test]
fn test_help_sessions_list() {
    insta::assert_snapshot!(snapshot_help(&["sessions", "list"]));
}

#[test]
fn test_help_sessions_show() {
    insta::assert_snapshot!(snapshot_help(&["sessions", "show"]));
}

#[test]
fn test_help_sessions_stats() {
    insta::assert_snapshot!(snapshot_help(&["sessions", "stats"]));
}

#[test]
fn test_help_sessions_messages() {
    insta::assert_snapshot!(snapshot_help(&["sessions", "messages"]));
}

#[test]
fn test_help_sessions_plans() {
    insta::assert_snapshot!(snapshot_help(&["sessions", "plans"]));
}

// package subcommands
#[test]
fn test_help_package_info() {
    insta::assert_snapshot!(snapshot_help(&["package", "info"]));
}

#[test]
fn test_help_package_list() {
    insta::assert_snapshot!(snapshot_help(&["package", "list"]));
}

#[test]
fn test_help_package_tree() {
    insta::assert_snapshot!(snapshot_help(&["package", "tree"]));
}

#[test]
fn test_help_package_why() {
    insta::assert_snapshot!(snapshot_help(&["package", "why"]));
}

#[test]
fn test_help_package_outdated() {
    insta::assert_snapshot!(snapshot_help(&["package", "outdated"]));
}

#[test]
fn test_help_package_audit() {
    insta::assert_snapshot!(snapshot_help(&["package", "audit"]));
}

// tools subcommands
#[test]
fn test_help_tools_lint() {
    insta::assert_snapshot!(snapshot_help(&["tools", "lint"]));
}

#[test]
fn test_help_tools_test() {
    insta::assert_snapshot!(snapshot_help(&["tools", "test"]));
}

// serve subcommands
#[test]
fn test_help_serve_mcp() {
    insta::assert_snapshot!(snapshot_help(&["serve", "mcp"]));
}

#[test]
fn test_help_serve_http() {
    insta::assert_snapshot!(snapshot_help(&["serve", "http"]));
}

#[test]
fn test_help_serve_lsp() {
    insta::assert_snapshot!(snapshot_help(&["serve", "lsp"]));
}

// generate subcommands
#[test]
fn test_help_generate_client() {
    insta::assert_snapshot!(snapshot_help(&["generate", "client"]));
}

#[test]
fn test_help_generate_types() {
    insta::assert_snapshot!(snapshot_help(&["generate", "types"]));
}

#[test]
fn test_help_generate_cli_snapshot() {
    insta::assert_snapshot!(snapshot_help(&["generate", "cli-snapshot"]));
}

#[test]
fn test_help_generate_typegen() {
    insta::assert_snapshot!(snapshot_help(&["generate", "typegen"]));
}

// rules subcommands
#[test]
fn test_help_rules_add() {
    insta::assert_snapshot!(snapshot_help(&["rules", "add"]));
}

#[test]
fn test_help_rules_list() {
    insta::assert_snapshot!(snapshot_help(&["rules", "list"]));
}

#[test]
fn test_help_rules_update() {
    insta::assert_snapshot!(snapshot_help(&["rules", "update"]));
}

#[test]
fn test_help_rules_remove() {
    insta::assert_snapshot!(snapshot_help(&["rules", "remove"]));
}
