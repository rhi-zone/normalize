# normalize-analyze

Shared entity types, ranking infrastructure, and table rendering for `normalize analyze` and `normalize rank` commands.

Key types: `Entity` trait, `FunctionEntity`, `ModuleEntity`, `FileEntity`, `Scored<E>`, `RankStats`. Key functions: `rank_pipeline` (sort + stats + truncate for `Scored<E>` lists) and `rank_and_truncate` (same for arbitrary `Vec<T>` with custom comparator). Also exports `truncate_path` used by ranked-list formatters to keep tabular output aligned. The `ranked` module provides the `RankEntry` trait, `DiffableRankEntry` trait, `Column`/`Align` types, and `format_ranked_table()` function: entry structs implement `RankEntry` to define columns and values, then call `format_ranked_table()` for shared tabular rendering across all rank commands.
