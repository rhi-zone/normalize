# normalize-analyze/src

Source for the `normalize-analyze` crate.

`lib.rs` defines the `Entity` trait and concrete entity types (`FunctionEntity`, `ModuleEntity`, `FileEntity`) plus the `truncate_path` helper. `ranked.rs` defines `Scored<E>`, `RankStats`, `rank_pipeline`, and `rank_and_truncate` — the shared sort/stats/truncate pipeline consumed by all ranked analyze commands.
