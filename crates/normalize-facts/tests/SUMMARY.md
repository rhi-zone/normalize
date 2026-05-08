# normalize-facts — integration tests

Integration tests for the `normalize-facts` crate's fact extraction pipeline.

Each subdirectory under `fixtures/` contains a minimal project per language, used to verify that
the fact extractor correctly identifies symbols, imports, and call relationships.

`extract_fixtures.rs` points `NORMALIZE_GRAMMAR_PATH` at the workspace's
`target/grammars/` directory before running, so tests use the workspace-built grammars
regardless of what the developer happens to have installed in
`~/.config/normalize/grammars/`. Run `cargo xtask build-grammars` once to populate
`target/grammars/`; the test panics with an actionable message if it's missing.

Set `UPDATE_FIXTURES=1` to regenerate `expected/` files from actual extractor output.
