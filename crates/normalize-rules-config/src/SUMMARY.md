# normalize-rules-config

Shared rule configuration types (`RulesConfig`, `RuleOverride`, `SarifTool`, `Severity`) used by all normalize rule engines.

Loaded from `[rules]` in `.normalize/config.toml`. Per-rule overrides under `[rules."rule-id"]`, external SARIF tools under `[[rules.sarif-tools]]`. `Severity` enum covers Error, Warning, Info, and Hint. `RuleOverride` includes generic fields (`severity`, `enabled`, `allow`, `tags`) and rule-specific fields — notably `filenames` (a `Vec<String>` accepted as a single string or list) used by `stale-summary`/`missing-summary` to configure which doc filenames are checked. Used by:
- `normalize-syntax-rules` — syntax (tree-sitter) rule loader
- `normalize-facts-rules-interpret` — Datalog fact rule loader
- `normalize-rules` — unified rule orchestration
