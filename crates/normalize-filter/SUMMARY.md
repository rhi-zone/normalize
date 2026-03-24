# normalize-filter

File filtering with glob patterns and alias resolution for `--exclude` and `--only` flags.

Key types: `Filter` (compiled include/exclude matchers backed by `ignore::gitignore`), `AliasConfig` (user-configurable alias map), `ResolvedAlias`, `AliasStatus`, `AliasResolution` (internal enum for alias lookup results). Built-in aliases: `@tests` (language-aware, delegates to `normalize-language-meta`), `@config`, `@build`, `@docs`, `@generated`. Bare language names passed to `--only`/`--exclude` (e.g. `--only rust`) are detected and emit a helpful error instead of silently matching nothing. Optional `config` feature adds `Merge` and `JsonSchema` derives for integration with normalize's config layer. Also exports `list_aliases` for `normalize filter aliases`. The standalone CLI binary (enabled with `cli` feature) exposes `MatchReport` and `AliasesReport` output types that implement `OutputFormatter`.
