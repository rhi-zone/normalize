Source files for the `normalize-semantic` crate.

- `lib.rs` — crate root; re-exports `EmbeddingsConfig`, `populate_embeddings`, `SearchHit`
- `config.rs` — `EmbeddingsConfig` struct (`[embeddings]` TOML section)
- `chunks.rs` — context window builder (`build_symbol_chunk`, `strip_doc_markers`) and `SymbolRow` type
- `embedder.rs` — fastembed `TextEmbedding` wrapper with `encode_vector`/`decode_vector`/`cosine_similarity` utilities
- `schema.rs` — SQLite DDL constants for the `embeddings` table and indices
- `store.rs` — async SQLite helpers: `ensure_schema`, `upsert_embedding`, `load_all_embeddings`, `delete_embeddings_for_path`
- `search.rs` — `rerank()`: cosine similarity + staleness penalty → sorted `Vec<SearchHit>`
- `populate.rs` — `populate_embeddings()`: walks the structural index, builds chunks, batches through the embedder, writes to store
- `service.rs` — `SearchReport`, `SearchResultEntry`, `run_search()` (called from `FactsService::search`)
