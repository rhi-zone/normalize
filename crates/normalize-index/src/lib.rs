//! Index acquisition and import-graph construction for normalize.
//!
//! This is the foundational "index enabler" crate: it lets any feature crate
//! (graph, architecture, rank, view, …) acquire the file index and read the
//! import graph **without** depending on the main `normalize` crate or its
//! monolithic `NormalizeConfig`. Acquisition functions take config slices
//! (`&IndexConfig`, `&WalkConfig`); the main crate provides thin wrappers that
//! bind them to `NormalizeConfig`.
//!
//! It also breaks the historical `graph ↔ architecture` cycle: [`build_import_graph`]
//! used to live in `normalize-architecture` (which depends on `normalize-graph`),
//! so `normalize-graph` consumers could not build the import graph. It now lives
//! here — the leaf both crates depend on — and `normalize-architecture`
//! re-exports it.

mod acquire;
mod config;
mod import_graph;

pub use acquire::{
    NO_IMPORT_DATA, ensure_ready, ensure_ready_or_warn, open, open_if_enabled, require_import_graph,
};
pub use config::IndexConfig;
pub use import_graph::{ImportGraph, build_import_graph};

// The normalize data-directory primitive lives in `normalize-facts` (the lowest
// crate that needs it — its `structure` service resolves the same path). Re-export
// it here so index consumers and the main crate get a single import surface.
pub use normalize_facts::get_normalize_dir;

// Re-export the index types consumers work with, so a dependent needs only
// `normalize-index` in scope.
pub use normalize_facts::{CallGraphStats, ChangedFiles, FileIndex, IndexedFile, SymbolMatch};
