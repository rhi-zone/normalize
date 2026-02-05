//! Rule pack interface for plugins.
//!
//! A rule pack is a collection of related rules compiled into a dylib.
//! Both builtin rules and user-defined rules use this same interface.

use crate::{Diagnostic, Relations};
use abi_stable::{
    StableAbi, declare_root_module_statics,
    library::RootModule,
    package_version_strings,
    sabi_types::VersionStrings,
    std_types::{RStr, RString, RVec},
};

/// Metadata about a rule pack.
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct RulePackInfo {
    /// Unique identifier (e.g., "normalize-builtins", "my-company-rules")
    pub id: RString,
    /// Human-readable name
    pub name: RString,
    /// Version string
    pub version: RString,
    /// Brief description
    pub description: RString,
    /// List of rule IDs provided by this pack
    pub rules: RVec<RString>,
}

impl RulePackInfo {
    /// Create rule pack info
    pub fn new(id: &str, name: &str, version: &str, description: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
            rules: RVec::new(),
        }
    }

    /// Add a rule ID to this pack
    pub fn with_rule(mut self, rule_id: &str) -> Self {
        self.rules.push(rule_id.into());
        self
    }
}

/// The root module type that dylib plugins must export.
///
/// Use `export_root_module!` macro to generate the necessary exports.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = RulePackRef)))]
#[sabi(missing_field(panic))]
pub struct RulePack {
    /// Get metadata about this rule pack
    #[sabi(last_prefix_field)]
    pub info: extern "C" fn() -> RulePackInfo,

    /// Run all rules in this pack against the given relations.
    /// Returns diagnostics for any issues found.
    pub run: extern "C" fn(relations: &Relations) -> RVec<Diagnostic>,

    /// Run a specific rule by ID.
    /// Returns None if the rule doesn't exist.
    pub run_rule: extern "C" fn(rule_id: RStr<'_>, relations: &Relations) -> RVec<Diagnostic>,
}

impl RootModule for RulePackRef {
    declare_root_module_statics! {RulePackRef}

    const BASE_NAME: &'static str = "normalize_rules";
    const NAME: &'static str = "normalize_rules";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}
