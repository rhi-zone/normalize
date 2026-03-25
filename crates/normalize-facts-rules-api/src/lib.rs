//! Data types for normalize rule evaluation.
//!
//! This crate defines the `Relations` input facts and `Diagnostic` output type used
//! by the fact rule engine. Rules run as interpreted `.dl` files via
//! `normalize-facts-rules-interpret`; there is no dynamic library loading.
//!
//! # Architecture
//!
//! ```text
//! normalize-facts (extraction) -> Relations (facts) -> Datalog engine -> Diagnostics
//! ```
//!
//! Facts are extracted from code by normalize-facts and passed to the Datalog engine.
//! Each rule evaluates over these relations and produces Diagnostics.

mod diagnostic;
mod relations;

pub use diagnostic::{Diagnostic, DiagnosticLevel, Location};
pub use relations::{
    AttributeFact, CallFact, ImplementsFact, ImportFact, IsImplFact, ParentFact, QualifierFact,
    Relations, SymbolFact, SymbolRangeFact, TypeMethodFact, VisibilityFact,
};

// Re-export ascent for rule implementors
pub use ascent;
