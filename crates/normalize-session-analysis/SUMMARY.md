# normalize-session-analysis

Session analytics computation for AI coding agent logs — computes metrics from parsed `Session` data produced by `normalize-chat-sessions`.

Key types: `SessionAnalysisReport` (the top-level report, renamed from `SessionAnalysis` to follow the `*Report` convention), `ToolStats` (call counts and error rates per tool), `TokenStats` (input/output/cache token totals), `ModelPricing` (cost estimation per model; now has per-version sonnet constants: `SONNET_4_5`, `SONNET_3_7`, `SONNET_3_5`, `SONNET_3`). Implements `OutputFormatter` for text and pretty output. Consumes `Session`/`Turn`/`Message`/`ContentBlock` from `normalize-chat-sessions`; outputs via `normalize-output`. Intentionally separate from parsing — what metrics matter is consumer-specific.
