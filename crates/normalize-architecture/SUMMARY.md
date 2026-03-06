# normalize-architecture

Pure algorithms and supporting types for architecture analysis of a codebase.

Builds an import graph from the `FileIndex` SQLite database and computes coupling (fan-in/fan-out/instability), hub modules, cross-imports, orphan modules, symbol hotspots, dependency cycles (DFS), longest import chains, layer flows, and layering compliance. Key types: `ImportGraph`, `ModuleCoupling`, `HubModule`, `Cycle`, `CrossImport`, `OrphanModule`, `LayerFlow`, `LayeringModuleResult`. Report structs and `OutputFormatter` impls live in the main `normalize` crate; this crate contains only the pure computation.
