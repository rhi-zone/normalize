# normalize-session-analysis/src

Source for session analytics.

- `lib.rs` — all analysis logic: `SessionAnalysisReport`, `ToolStats`, `TokenStats`, `ModelPricing`; computation functions that walk `Session` turns and messages to aggregate metrics; `OutputFormatter` implementations for text and pretty display. `CorrectionKind` (renamed from `CorrectionType`) enumerates self-correction categories detected in assistant text. `SessionAnalysisReport::aggregate` folds N per-session reports into one aggregate (summing stats, merging command/retry patterns, re-ranking largest tool results) — the aggregation semantics live with the model.
