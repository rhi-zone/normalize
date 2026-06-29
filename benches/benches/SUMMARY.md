Criterion benchmark target files for the normalize workspace. Three targets:
- `structure_rebuild.rs` — benchmarks `SymbolParser::parse_file()` per-file hot path and `FileIndex::refresh()` full rebuild on a small crate.
- `rules_runner.rs` — benchmarks `build_relations_from_index()` loading all facts from SQLite into memory (requires an existing index; run `normalize structure rebuild` first).
- `cli_commands.rs` — benchmarks end-to-end `normalize view` and `normalize rank complexity` via process invocation (requires `cargo build` first).

Results land in `target/criterion/`. Baseline numbers are recorded in `docs/perf-baseline.md`.
