# normalize-facts — integration tests

Integration tests for the `normalize-facts` crate's fact extraction pipeline.

Each subdirectory under `fixtures/` contains a minimal project per language, used to verify that
the fact extractor correctly identifies symbols, imports, and call relationships.

`extract_fixtures.rs` probes each fixture's required tree-sitter grammars (the primary language
plus any other languages referenced by files in the project, e.g. `markdown` for `SUMMARY.md`,
`toml` for `Cargo.toml`) before running. Cases whose grammars aren't installed locally are
skipped with a warning rather than failing — CI builds all 99 grammars via
`cargo xtask build-grammars` and runs them all; dev machines typically have a subset.

Set `UPDATE_FIXTURES=1` to regenerate `expected/` files from actual extractor output (only for
fixtures whose grammars are available).
