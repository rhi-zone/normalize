# normalize-cfg/src

Control flow graph implementation.

- `lib.rs` — data model: `Cfg`, `BasicBlock`, `Edge`, `BlockId`, `FunctionId`, `BlockKind`, `EdgeKind`, `DefSite`, `UseSite`; re-exports `builder`, `mermaid`, `service`
- `builder.rs` — structured-CFG builder; takes a tree-sitter `Tree`, a `.cfg.scm` query string, and a body byte range; walks CST nodes classified by capture names to build the CFG graph; also extracts `@cfg.def`/`@cfg.use` captures and assigns them to the enclosing block
- `mermaid.rs` — Mermaid `flowchart TD` renderer; `render(cfg)` produces a human-readable flowchart; block shapes follow Mermaid conventions (stadium/box/diamond/trapezoid)
- `service.rs` — CLI service (`CfgService`) implementing `normalize cfg`; `CfgReport` with `OutputFormatter`; locates function body via tags query then builds and renders the CFG
