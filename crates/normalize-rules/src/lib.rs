//! Rule orchestration for normalize: syntax rules, fact rules, native checks, and SARIF engines.
//!
//! This crate owns all rule management logic extracted from the main `normalize` crate:
//! - `runner` — unified run, list, show, tags, enable/disable, add/update/remove
//! - `cmd_rules` — syntax rule runner (tree-sitter based)
//! - `loader` — diagnostic formatting helpers for fact rules
//! - `service` — `RulesService` with `#[cli]` registration (feature-gated)
//!
//! The `RulesRunConfig` struct allows callers to pass rule config without depending on
//! `normalize`'s `NormalizeConfig` (which would create a circular dependency).

pub mod cmd_rules;
pub mod loader;
pub mod runner;

#[cfg(feature = "cli")]
pub mod service;
#[cfg(feature = "cli")]
pub mod setup;

pub use runner::{
    ListFilters, RuleEntry, RuleInfoReport, RuleKind, RuleOverride, RulesConfig, RulesListReport,
    RulesRunConfig, RulesTagsReport, SarifTool, TagEntry, abi_diagnostic_to_issue, add_rule,
    apply_native_rules_config, build_list_report, build_relations_from_index,
    collect_fact_diagnostics, collect_fact_diagnostics_incremental, enable_disable,
    finding_to_issue, list_tags, list_tags_structured, remove_rule, run_rules_report,
    run_sarif_tools, show_rule, show_rule_structured, update_rules,
};

pub use loader::format_diagnostic;

#[cfg(feature = "cli")]
pub use service::{
    CompileError, CompileWarning, RuleShowReport, RulesCompileReport, RulesService,
    RulesValidateReport, load_rules_config,
};
