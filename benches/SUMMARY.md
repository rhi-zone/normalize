Criterion benchmark suite for normalize's hot paths. Three bench targets:
- `structure_rebuild` — `SymbolParser::parse_file()` per-file hot path and `FileIndex::refresh()` full rebuild on a small crate
- `rules_runner` — `build_relations_from_index()` loading all facts from SQLite into memory (requires an existing index; run `normalize structure rebuild` first)
- `cli_commands` — end-to-end `normalize view` and `normalize rank complexity` via process invocation (requires `cargo build` first)

Run with `cargo bench` from the workspace root. Results land in `target/criterion/`. Baseline numbers are recorded in `docs/perf-baseline.md`.
