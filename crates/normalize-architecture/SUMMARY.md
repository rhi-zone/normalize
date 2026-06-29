# normalize-architecture

Pure algorithms and supporting types for architecture analysis of a codebase.

Builds an import graph from the `FileIndex` SQLite database and computes coupling (fan-in/fan-out/instability), hub modules, cross-imports, orphan modules, symbol hotspots, dependency cycles (DFS), longest import chains, layer flows, and layering compliance. Key types: `ImportGraph`, `ModuleCoupling`, `HubModule`, `Cycle`, `CrossImport`, `OrphanModule`, `LayerFlow`, `LayeringModuleResult`. Report structs and `OutputFormatter` impls live in the main `normalize` crate; this crate contains only the pure computation. Single-module crate (all logic in `lib.rs`). Published as a standalone crate on crates.io (part of the 38-crate normalize workspace). Consumers (`analyze architecture`, `view graph`) require a non-empty import graph — they go through `index::require_import_graph` in the main crate and error rather than emit a zeroed report when no imports are indexed.
