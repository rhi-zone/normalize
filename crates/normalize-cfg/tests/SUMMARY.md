# normalize-cfg/tests

Integration and snapshot tests for the CFG builder and Mermaid renderer.

- `rust_cfg.rs` — snapshot tests for the Rust CFG builder; one test per fixture in `fixtures/rust/`; Mermaid output is snapshot-tested with `insta`
- `fixtures/` — small source files for testing (one per language/control-flow pattern)
- `snapshots/` — insta snapshot files (auto-generated; update with `cargo insta review`)
