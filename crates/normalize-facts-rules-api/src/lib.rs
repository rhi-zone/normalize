//! Stable ABI for normalize rule plugins.
//!
//! This crate defines the interface between the normalize engine and rule plugins.
//! Rules are compiled to dylibs and loaded at runtime, enabling:
//! - Independent updates of builtins without engine changes
//! - User-defined rule packs with the same infrastructure
//! - Sharing of rule packs between users
//!
//! # Architecture
//!
//! ```text
//! normalize-facts (extraction) -> Relations (facts) -> RulePack (rules) -> Diagnostics
//! ```
//!
//! Facts are extracted from code by normalize-facts and passed to rule packs as Relations.
//! Each rule pack applies Datalog rules over these relations and produces Diagnostics.

mod diagnostic;
mod relations;
mod rule_pack;

pub use diagnostic::{Diagnostic, DiagnosticLevel, Location};
pub use relations::{CallFact, ImportFact, Relations, SymbolFact};
pub use rule_pack::{RulePack, RulePackInfo, RulePackRef};

// Re-export ascent for rule implementors
pub use ascent;

// Re-export abi_stable types needed by plugins
pub use abi_stable::{
    StableAbi, export_root_module,
    prefix_type::PrefixTypeTrait,
    sabi_extern_fn, sabi_trait,
    std_types::{RStr, RString, RVec},
};
