Source files for the `normalize-semantic` crate.

- `lib.rs` — crate root; re-exports `EmbeddingsConfig`, `populate_embeddings`, `populate_markdown_docs`, `populate_commit_messages`, `populate_incremental_for_paths`, `DEFAULT_MAX_COMMITS`, `SearchHit`
- `config.rs` — `EmbeddingsConfig` struct (`[embeddings]` TOML section)
- `chunks.rs` — context window builders: `build_symbol_chunk` (code symbols), `build_markdown_chunk` + `split_markdown_sections` (markdown heading sections), `build_commit_chunk` (git commits); plus `SymbolRow` type
- `embedder.rs` — fastembed `TextEmbedding` wrapper with `encode_vector`/`decode_vector`/`cosine_similarity`/`dims_for_model` utilities
- `schema.rs` — SQLite DDL constants for the `embeddings` table (with UNIQUE constraint on `(source_type, source_path, source_id)`), indices, drop statements, and `vec_embeddings` ANN virtual table (`create_vec_embeddings_ddl`)
- `store.rs` — async SQLite helpers: `ensure_schema`, `ensure_vec_schema`, `upsert_embedding` (`INSERT OR REPLACE`, syncs to `vec_embeddings` via `VecConnection`), `ann_search`, `load_all_embeddings`, `delete_embeddings_for_path`, `drop_embedding_tables`, `vacuum`; vec-specific operations accept `Option<&VecConnection>` for per-connection sqlite-vec support
- `search.rs` — `rerank()`: cosine similarity + staleness penalty → sorted `Vec<SearchHit>`; used for both ANN candidate re-ranking and brute-force fallback
- `git_staleness.rs` — `compute_staleness_batch()`: computes per-file staleness scores from git history; cached per unique file path to avoid redundant walks
- `populate.rs` — `populate_embeddings()`: symbols (full/incremental); `populate_markdown_docs()`: SUMMARY.md, CLAUDE.md, README.md, docs/*.md chunked by heading section; `populate_commit_messages()`: last N commits via gix, incremental (skips already-embedded hashes); `populate_incremental_for_paths()`: daemon hook for re-embedding on file change (no VACUUM, no full table drop)
- `service.rs` — `SearchReport` (includes `ann_used` flag), `SearchResultEntry`, `run_search()` — opens a `VecConnection` for ANN queries, falls back to brute-force when unavailable; results include all source types (symbol, doc, commit)
- `vec_ext.rs` — per-connection sqlite-vec registration: `VecConnection` opens a raw FFI handle to the database and calls `sqlite3_vec_init` directly (avoids `sqlite3_auto_extension` which conflicts with libsql's initialization); `VecStmt` provides prepared statement helpers over raw FFI
