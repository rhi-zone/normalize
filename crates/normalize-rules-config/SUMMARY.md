# normalize-rules-config

Minimal crate providing the canonical `RulesConfig`, `RuleOverride`, and `SarifTool` types shared
across all normalize rule engines. No rule logic — just the configuration schema.
Deserialized from `[rules]` in `.normalize/config.toml`.
