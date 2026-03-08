# normalize-rules-config

Shared rule configuration types (`RulesConfig`, `RuleOverride`, `SarifTool`) used by all normalize rule engines.

Loaded from `[rules]` in `.normalize/config.toml`. Per-rule overrides under `[rules."rule-id"]`, external SARIF tools under `[[rules.sarif-tools]]`. Used by:
- `normalize-syntax-rules` — syntax (tree-sitter) rule loader
- `normalize-facts-rules-interpret` — Datalog fact rule loader
- `normalize-rules` — unified rule orchestration
