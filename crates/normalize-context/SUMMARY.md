# normalize-context

Frontmatter-filtered context resolution for normalize: hierarchical `.normalize/context/` walk with YAML frontmatter matching. Walks from the project directory up to the filesystem root, then includes the global `~/.normalize/` layer. Each `.md` file may contain one or more blocks with optional YAML frontmatter; blocks are filtered against caller-provided context (`CallerContext` — a flat dot-path map). Supports match strategies: `equals`, `contains`, `keywords`, `regex`, `exists`, `one_of`; composable `conditions: all:/any:` in frontmatter. Published as a standalone library crate. The `cli` feature flag is reserved for future CLI-specific extensions.

- `Cargo.toml` — crate manifest; `cli` feature reserved for CLI-specific deps; deps: `serde`, `serde_json`, `serde_yaml`, `dirs`, `regex`, `schemars`
- `src/` — implementation (see `src/SUMMARY.md`)
