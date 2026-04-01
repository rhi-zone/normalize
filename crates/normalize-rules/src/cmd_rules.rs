//! Syntax rule execution helpers.
//!
//! Contains `run_syntax_rules`, which loads and runs tree-sitter-based syntax rules
//! and returns raw `Finding` values for further processing by callers.

use normalize_languages::parsers::grammar_loader;
use normalize_syntax_rules::{DebugFlags, Finding, RulesConfig, load_all_rules, run_rules};
use std::collections::HashSet;
use std::path::Path;

/// Run syntax rules and return raw findings (no printing).
#[allow(clippy::too_many_arguments)]
pub fn run_syntax_rules(
    root: &Path,
    project_root: &Path,
    filter_rule: Option<&str>,
    filter_tag: Option<&str>,
    filter_ids: Option<&HashSet<String>>,
    config: &RulesConfig,
    debug: &DebugFlags,
    files: Option<&[std::path::PathBuf]>,
    path_filter: &normalize_rules_config::PathFilter,
    walk_config: &normalize_rules_config::WalkConfig,
) -> Vec<Finding> {
    let rules = load_all_rules(project_root, config);
    if rules.is_empty() {
        return Vec::new();
    }
    let loader = grammar_loader();
    run_rules(
        &rules,
        root,
        project_root,
        &loader,
        filter_rule,
        filter_tag,
        filter_ids,
        debug,
        files,
        path_filter,
        walk_config,
    )
}
