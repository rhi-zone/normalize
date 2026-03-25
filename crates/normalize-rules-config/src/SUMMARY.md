# normalize-rules-config

Shared rule configuration types (`RulesConfig`, `RuleOverride`, `SarifTool`, `Severity`) used by all normalize rule engines.

Loaded from `[rules]` in `.normalize/config.toml`. Per-rule overrides under `[rules."rule-id"]`, external SARIF tools under `[[rules.sarif-tools]]`. `Severity` enum covers Error, Warning, Info, and Hint. `RuleOverride` includes generic fields (`severity`, `enabled`, `allow`, `tags`) and rule-specific fields — `filenames` (a `Vec<String>` accepted as a single string or list, used by `stale-summary`/`missing-summary` to configure which doc filenames are checked) and `paths` (a `Vec<String>` accepted as a single string or list, used by `stale-summary`/`missing-summary` to scope coverage to matching directory globs; empty means check everywhere). Both `filenames` and `paths` use the same `deserialize_one_or_many` helper. Used by:
- `normalize-syntax-rules` — syntax (tree-sitter) rule loader
- `normalize-facts-rules-interpret` — Datalog fact rule loader
- `normalize-rules` — unified rule orchestration
