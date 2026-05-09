# normalize-cfg

Control flow graph (CFG) builder and renderer for normalize. Builds a structured CFG from a tree-sitter parse tree using `.cfg.scm` queries; renders to Mermaid flowcharts. Exposes a `normalize cfg` CLI subcommand via the `cli` feature flag.

- `Cargo.toml` — crate manifest; `cli` feature gates `server-less` dependency
- `src/` — implementation (see `src/SUMMARY.md`)
