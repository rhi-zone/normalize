# normalize-analyze

Shared entity types and ranking infrastructure for `normalize analyze` commands.

Key types: `Entity` trait, `FunctionEntity`, `ModuleEntity`, `FileEntity`, `Scored<E>`, `RankStats`. Key functions: `rank_pipeline` (sort + stats + truncate for `Scored<E>` lists) and `rank_and_truncate` (same for arbitrary `Vec<T>` with custom comparator). Also exports `truncate_path` used by ranked-list formatters to keep tabular output aligned.

**Table rendering infrastructure** (Phase 3 rank consolidation): `RankEntry` trait + `Column`/`Align` types + `format_ranked_table()` function. Entry structs implement `RankEntry` to define columns and values, then call `format_ranked_table()` for shared tabular rendering. Used by: files, imports, ownership, docs, ceremony, surface, depth-map, layering.
