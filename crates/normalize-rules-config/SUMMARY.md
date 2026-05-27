# normalize-rules-config

Minimal crate providing the canonical `RulesConfig`, `RuleOverride`, `Severity`, and `SarifTool`
types shared across all normalize rule engines. No rule logic — just the configuration schema.
Deserialized from `[rules]` in `.normalize/config.toml`.

`Severity` (Error / Warning / Info) is defined here and re-exported by both `normalize-syntax-rules`
and `normalize-facts-rules-interpret` so all rule engines share a single definition.
`RuleOverride::merge` and `RulesConfig::merge` use a "right-wins" strategy: `other`'s
present fields override `self`'s, but absent `Option` fields in `other` leave `self`'s
values unchanged.

Also exports `ConfigDiff` — a tiered change classifier used by the daemon to pick the
cheapest correct cache-invalidation strategy (filter-only / per-rule re-run / full reprime)
when `.normalize/config.toml` changes.

`WalkConfig` includes `with_daemon_baseline()` — a safety method that unconditionally adds
`.git/` and `.normalize/` to the exclusion list. Daemon index walkers call this after loading
config to prevent descending into `.normalize/` when a project config exists but omits `[walk]`.
