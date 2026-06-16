# normalize-analyze/src

Source for the `normalize-analyze` crate.

`lib.rs` defines the `Entity` trait and concrete entity types (`FunctionEntity`, `ModuleEntity`, `FileEntity`) plus the `truncate_path` helper. `ranked.rs` provides four layers of infrastructure: (1) scoring/sorting via `Scored<E>`, `RankStats`, `rank_pipeline`, and `rank_and_truncate`; (2) table rendering via the `RankEntry` trait, `Column`/`Align` types, and `format_ranked_table()`; (3) diff support via the `DiffableRankEntry` trait, `compute_ranked_diff()`, and `format_delta()` — implement `DiffableRankEntry` on entry structs to enable `--diff <ref>` comparison against git baselines; (4) the `RiskTier` severity enum (`Low`/`Moderate`/`High`/`Critical` with `title()` + `rank()`) — the shared vocabulary for the `Risk` column in `complexity`/`length`/`test-gaps` (replaces the old `### Critical` subsections). Color mapping lives consumer-side in `normalize::output::tier_color` to keep this crate color-free.

The `rank` output house style every subcommand conforms to is documented in `docs/cli-design.md` ("Rank output house style").
