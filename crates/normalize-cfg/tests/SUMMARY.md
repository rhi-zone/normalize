# normalize-cfg/tests

Integration and snapshot tests for the CFG builder and Mermaid renderer.

- `rust_cfg.rs` — snapshot tests for the Rust CFG builder; one test per fixture in `fixtures/rust/`; Mermaid output is snapshot-tested with `insta`
- `python_cfg.rs` — snapshot tests for the Python CFG builder; skips gracefully if grammar not installed
- `go_cfg.rs` — snapshot tests for the Go CFG builder; skips gracefully if grammar not installed
- `fixtures/` — small source files for testing (one per language/control-flow pattern)
- `snapshots/` — insta snapshot files (auto-generated; update with `cargo insta review`)
