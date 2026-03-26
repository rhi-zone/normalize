# Performance Baseline — 0.3.0

Established 2026-03-25 on the normalize repo (~38 crates, real-world size).

**How to reproduce:**
```
cargo build
normalize structure rebuild  # required for rules_runner bench
cargo bench
```

## Results

### structure_rebuild/per_file

| Benchmark | Median | Std Dev | Notes |
|---|---|---|---|
| parse_file(index.rs) | 32.8 ms | ~1.6 ms | index.rs ~2000 lines, largest hot file |

### structure_rebuild/full_refresh

| Benchmark | Median | Std Dev | Notes |
|---|---|---|---|
| FileIndex::refresh(normalize-facts-core) | 246.5 ms | ~3.5 ms | Small crate, ~10 source files |

### rules_runner

| Benchmark | Median | Std Dev | Notes |
|---|---|---|---|
| build_relations_from_index(normalize) | 362.9 µs | ~6 µs | Loads all symbols/imports/calls from SQLite |

### cli_commands

| Benchmark | Median | Std Dev | Notes |
|---|---|---|---|
| normalize view index.rs | 2.12 s | ~30 ms | Includes binary startup + parse, no index |
| normalize rank complexity normalize-facts/src | 142.4 ms | ~1.5 ms | Single-threaded walk+parse, small crate |

## Known Hot Spots (from code audit)

These are the suspected top allocators to confirm with `heaptrack`/`massif`:

1. **`Vec<ParsedFileData>` accumulation** (`index.rs:1916-1993`) — all parsed facts buffered in memory before DB insert. Fix: stream inserts (insert per-file as parsing completes).
2. **`Relations` struct** (`runner.rs:1505-1591`) — loads ALL symbols, imports, calls into memory for every rules run. Fix: query only what each rule needs, or use incremental loading.
3. **Binary file reads for line counting** (`index.rs:822`) — reads binary files via `read_to_string()`. Fix: skip non-UTF-8 files (detect via magic bytes or `read_to_string` error).

## Memory Profiling

Run with heaptrack:
```
heaptrack ./target/debug/normalize structure rebuild
heaptrack-gui heaptrack.normalize.*.gz
```

Or with massif:
```
valgrind --tool=massif ./target/debug/normalize structure rebuild
ms_print massif.out.*
```
