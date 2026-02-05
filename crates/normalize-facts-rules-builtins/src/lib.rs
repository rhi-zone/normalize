//! Builtin rules for normalize.
//!
//! This crate provides the default rule pack that ships with normalize.
//! Rules are implemented using Ascent (Datalog) for declarative pattern matching.

mod circular_deps;

use abi_stable::{export_root_module, prefix_type::WithMetadata, sabi_extern_fn};
use normalize_facts_rules_api::{
    Diagnostic, RStr, RVec, Relations, RulePack, RulePackInfo, RulePackRef,
};

/// Get information about this rule pack
#[sabi_extern_fn]
fn info() -> RulePackInfo {
    RulePackInfo::new(
        "normalize-builtins",
        "Normalize Builtin Rules",
        env!("CARGO_PKG_VERSION"),
        "Default rules for architectural analysis",
    )
    .with_rule("circular-deps")
}

/// Run all rules
#[sabi_extern_fn]
fn run(relations: &Relations) -> RVec<Diagnostic> {
    let mut diagnostics = RVec::new();

    // Run circular dependency detection
    diagnostics.extend(circular_deps::run(relations));

    diagnostics
}

/// Run a specific rule by ID
#[sabi_extern_fn]
fn run_rule(rule_id: RStr<'_>, relations: &Relations) -> RVec<Diagnostic> {
    match rule_id.as_str() {
        "circular-deps" => circular_deps::run(relations).into_iter().collect(),
        _ => RVec::new(),
    }
}

/// The static rule pack instance with metadata for ABI stability
static RULE_PACK: WithMetadata<RulePack> = WithMetadata::new(RulePack {
    info,
    run,
    run_rule,
});

/// Export the rule pack for dynamic loading
///
/// # Safety
/// The static RULE_PACK is correctly initialized and remains valid for 'static.
#[export_root_module]
pub fn get_rule_pack() -> RulePackRef {
    // SAFETY: RULE_PACK is a const-initialized static with correct layout
    RulePackRef(unsafe { RULE_PACK.as_prefix() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info() {
        let info = info();
        assert_eq!(info.id.as_str(), "normalize-builtins");
        assert!(!info.rules.is_empty());
    }

    #[test]
    fn test_run_empty() {
        let relations = Relations::new();
        let diagnostics = run(&relations);
        assert!(diagnostics.is_empty());
    }
}
