# normalize-tools/src/adapters

Per-tool `Tool` trait implementations for all built-in external tools.

14 adapters (all feature-gated): Python — `ruff.rs`, `mypy.rs`, `pyright.rs`; JavaScript/TypeScript — `oxlint.rs`, `oxfmt.rs`, `eslint.rs`, `biome.rs` (BiomeLint + BiomeFormat), `prettier.rs`, `tsc.rs`, `tsgo.rs`, `deno.rs`; Rust — `clippy.rs`, `rustfmt.rs`; Go — `gofmt.rs` (Gofmt + Govet). Each adapter detects availability via `which`, scores project relevance by checking config files and source extensions, runs the tool subprocess, and parses its output into `Vec<Diagnostic>`. Shared helper `parse_ts_compiler_output` is used by both tsc and tsgo. `mod.rs` exports `all_adapters()`.
