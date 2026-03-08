# normalize-rules-config

Shared rule configuration types (`RulesConfig`, `RuleOverride`) used by all normalize rule engines.

Loaded from `[analyze.rules]` in `.normalize/config.toml`. Used by:
- `normalize-syntax-rules` — syntax (tree-sitter) rule loader
- `normalize-facts-rules-interpret` — Datalog fact rule loader
- `normalize-rules` — unified rule orchestration
