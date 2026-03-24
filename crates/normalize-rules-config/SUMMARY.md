# normalize-rules-config

Minimal crate providing the canonical `RulesConfig`, `RuleOverride`, `Severity`, and `SarifTool`
types shared across all normalize rule engines. No rule logic — just the configuration schema.
Deserialized from `[rules]` in `.normalize/config.toml`.

`Severity` (Error / Warning / Info) is defined here and re-exported by both `normalize-syntax-rules`
and `normalize-facts-rules-interpret` so all rule engines share a single definition.
`RuleOverride::merge` and `RulesConfig::merge` use a "right-wins" strategy: `other`'s
present fields override `self`'s, but absent `Option` fields in `other` leave `self`'s
values unchanged.
