Semantic retrieval layer for normalize.

Provides vector embeddings over structurally-derived chunks (symbols, doc comments,
callers/callees, co-change neighbors), stored in SQLite alongside the structural
index, queryable by meaning rather than by name.

## Key modules

- `config.rs` — `EmbeddingsConfig` (`[embeddings]` section of `.normalize/config.toml`)
- `chunks.rs` — context window construction: symbol name + signature + doc comment + callers/callees + co-change neighbors
- `embedder.rs` — fastembed wrapper (ONNX-backed, no server required); supports `nomic-embed-text-v1.5` (768-dim, default), `all-MiniLM-L6-v2` (384-dim), and `all-MiniLM-L12-v2`
- `schema.rs` — SQLite DDL for the `embeddings` table (migrates-in alongside existing schema)
- `store.rs` — read/write embeddings to/from SQLite (upsert, bulk-load, delete-by-path)
- `search.rs` — brute-force ANN search + staleness re-ranking (`score = cosine_sim * (1 - 0.3 * staleness)`)
- `populate.rs` — walks all symbols from the structural index, builds context windows, and embeds in batches
- `service.rs` — `SearchReport` type and `run_search()` function called from `FactsService::search` in the main crate

## Exposed surface

- `normalize structure search <query>` — natural-language semantic search over the indexed codebase
- `normalize structure rebuild` — population is wired in automatically when `embeddings.enabled = true`
- `NormalizeConfig.embeddings` — `[embeddings]` section: `enabled` (bool, default false), `model` (string, default `nomic-embed-text-v1.5`)

## TTY behaviour

- TTY + not configured: print suggestion to enable, exit with error
- TTY + enabled + model missing: fastembed downloads with progress
- Non-TTY + enabled + model missing: silent download (it's configured)
- Non-TTY + not configured: clear error to stderr, exit non-zero
