# normalize-analyze/src

Source for the `normalize-analyze` crate.

`lib.rs` defines the `Entity` trait and concrete entity types (`FunctionEntity`, `ModuleEntity`, `FileEntity`) plus the `truncate_path` helper. `ranked.rs` provides three layers of infrastructure: (1) scoring/sorting via `Scored<E>`, `RankStats`, `rank_pipeline`, and `rank_and_truncate`; (2) table rendering via the `RankEntry` trait, `Column`/`Align` types, and `format_ranked_table()`; (3) diff support via the `DiffableRankEntry` trait, `compute_ranked_diff()`, and `format_delta()` — implement `DiffableRankEntry` on entry structs to enable `--diff <ref>` comparison against git baselines.
