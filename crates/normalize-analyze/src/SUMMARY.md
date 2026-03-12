# normalize-analyze/src

Source for the `normalize-analyze` crate.

`lib.rs` defines the `Entity` trait and concrete entity types (`FunctionEntity`, `ModuleEntity`, `FileEntity`) plus the `truncate_path` helper. `ranked.rs` provides two layers of infrastructure: (1) scoring/sorting via `Scored<E>`, `RankStats`, `rank_pipeline`, and `rank_and_truncate`; (2) table rendering via the `RankEntry` trait, `Column`/`Align` types, and `format_ranked_table()` — implement `RankEntry` on your entry struct and call `format_ranked_table()` in your `OutputFormatter::format_text()` for shared tabular output.
