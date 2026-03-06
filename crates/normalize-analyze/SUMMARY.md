# normalize-analyze

Shared entity types and ranking infrastructure for `normalize analyze` commands.

Key types: `Entity` trait, `FunctionEntity`, `ModuleEntity`, `FileEntity`, `Scored<E>`, `RankStats`. Key functions: `rank_pipeline` (sort + stats + truncate for `Scored<E>` lists) and `rank_and_truncate` (same for arbitrary `Vec<T>` with custom comparator). Also exports `truncate_path` used by ranked-list formatters to keep tabular output aligned.
