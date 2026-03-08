//! Rule orchestration for normalize: syntax rules, fact rules, native checks, and SARIF engines.
//!
//! This crate owns all rule management logic extracted from the main `normalize` crate:
//! - `runner` — unified run, list, show, tags, enable/disable, add/update/remove
//! - `cmd_rules` — syntax rule runner (tree-sitter based)
//! - `service` — `RulesService` with `#[cli]` registration (feature-gated)
//!
//! The `RulesRunConfig` struct allows callers to pass rule config without depending on
//! `normalize`'s `NormalizeConfig` (which would create a circular dependency).

pub mod cmd_rules;
pub mod runner;

#[cfg(feature = "cli")]
pub mod service;

pub use runner::{
    ListFilters, RuleEntry, RuleOverride, RuleType, RulesConfig, RulesListReport, RulesRunConfig,
    SarifTool, abi_diagnostic_to_issue, add_rule, apply_native_rules_config, build_list_report,
    build_relations_from_index, collect_fact_diagnostics, enable_disable, finding_to_issue,
    list_tags, remove_rule, run_rules_report, run_sarif_tools, show_rule, update_rules,
};

#[cfg(feature = "cli")]
pub use service::{RuleResult, RulesService, load_rules_config};
