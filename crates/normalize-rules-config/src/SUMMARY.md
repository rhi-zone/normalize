# normalize-rules-config

Shared rule configuration types (`RulesConfig`, `RuleOverride`, `SarifTool`, `Severity`) used by all normalize rule engines.

Loaded from `[rules]` in `.normalize/config.toml`. Per-rule overrides under `[rules."rule-id"]`, external SARIF tools under `[[rules.sarif-tools]]`. `Severity` enum covers Error, Warning, Info, and Hint. Used by:
- `normalize-syntax-rules` — syntax (tree-sitter) rule loader
- `normalize-facts-rules-interpret` — Datalog fact rule loader
- `normalize-rules` — unified rule orchestration
