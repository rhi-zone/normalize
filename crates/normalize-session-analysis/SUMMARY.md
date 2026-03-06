# normalize-session-analysis

Session analytics computation for AI coding agent logs — computes metrics from parsed `Session` data produced by `normalize-chat-sessions`.

Key types: `SessionAnalysis` (the top-level report), `ToolStats` (call counts and error rates per tool), `TokenStats` (input/output/cache token totals), `ModelPricing` (cost estimation per model). Implements `OutputFormatter` for text and pretty output. Consumes `Session`/`Turn`/`Message`/`ContentBlock` from `normalize-chat-sessions`; outputs via `normalize-output`. Intentionally separate from parsing — what metrics matter is consumer-specific.
