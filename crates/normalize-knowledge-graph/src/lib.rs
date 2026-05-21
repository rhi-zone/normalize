//! normalize-knowledge-graph: persistent, addressable, queryable knowledge graph.
//!
//! Provides **Unit** (addressable markdown document with YAML frontmatter),
//! **Edge** (directed, typed relationship between units), and **Query** (predicate
//! matching with dotted-path metadata lookup and BFS traversal).
//!
//! Storage root: `.normalize/kg/` (resolved via the same env-var logic as
//! `normalize::paths::get_normalize_dir`, inlined to avoid circular deps).
//! Units: `<id>.md`. Outgoing edges are stored in each unit's `links` frontmatter
//! field — no shared mutable log. Legacy `edges.jsonl` logs are auto-migrated on
//! first use and renamed to `edges.jsonl.migrated-v0`.

pub mod model;
pub mod query;
pub mod reports;
pub mod store;

#[cfg(feature = "cli")]
pub mod service;
