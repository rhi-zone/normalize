# normalize-filter

File filtering with glob patterns and alias resolution for `--exclude` and `--only` flags.

Key types: `Filter` (compiled include/exclude matchers backed by `ignore::gitignore`), `AliasConfig` (user-configurable alias map), `ResolvedAlias`, `AliasStatus`. Built-in aliases: `@tests` (language-aware, delegates to `normalize-language-meta`), `@config`, `@build`, `@docs`, `@generated`. Optional `config` feature adds `Merge` and `JsonSchema` derives for integration with normalize's config layer. Also exports `list_aliases` for `normalize filter aliases`.

No changes in this release cycle — touched to reset staleness counter (2026-03-24).
