Source files for the `normalize-semantic` crate.

- `lib.rs` — crate root; re-exports `EmbeddingsConfig`, `populate_embeddings`, `SearchHit`
- `config.rs` — `EmbeddingsConfig` struct (`[embeddings]` TOML section)
- `chunks.rs` — context window builder (`build_symbol_chunk`) and `SymbolRow` type; doc comment text comes from the index (already marker-stripped by `Language::extract_docstring`)
- `embedder.rs` — fastembed `TextEmbedding` wrapper with `encode_vector`/`decode_vector`/`cosine_similarity`/`dims_for_model` utilities
- `schema.rs` — SQLite DDL constants for the `embeddings` table (with UNIQUE constraint on `(source_type, source_path, source_id)`), indices, drop statements, and `vec_embeddings` ANN virtual table (`create_vec_embeddings_ddl`)
- `store.rs` — async SQLite helpers: `ensure_schema`, `ensure_vec_schema`, `upsert_embedding` (`INSERT OR REPLACE`, syncs to `vec_embeddings`), `ann_search`, `load_all_embeddings`, `delete_embeddings_for_path`, `drop_embedding_tables`, `vacuum`
- `search.rs` — `rerank()`: cosine similarity + staleness penalty → sorted `Vec<SearchHit>`; used for both ANN candidate re-ranking and brute-force fallback
- `git_staleness.rs` — `compute_staleness_batch()`: computes per-file staleness scores from git history; cached per unique file path to avoid redundant walks
- `populate.rs` — `populate_embeddings()`: walks the structural index, builds chunks, batches through the embedder, writes to store; full rebuild drops+recreates tables and VACUUMs; incremental uses `INSERT OR REPLACE`; emits progress to stderr
- `service.rs` — `SearchReport` (includes `ann_used` flag), `SearchResultEntry`, `run_search()` — uses ANN path when `vec_embeddings` is available, falls back to brute-force
- `vec_ext.rs` — sqlite-vec extension registration: `register_vec_extension()` calls `sqlite3_auto_extension` once per process to make `vec0` tables available on all subsequent connections
