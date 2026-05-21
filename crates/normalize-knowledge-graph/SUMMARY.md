Persistent knowledge graph adjacent to code. Exposes three CLI primitives: `read` (selector → units), `write` (jq transform → mutate/delete), `walk` (BFS graph traversal via jq-extracted link targets). Storage in `.normalize/kg/` — unit files as `<id>.md` with YAML frontmatter; outgoing edges in each unit's `links` field. Auto-migrates legacy `edges.jsonl` logs on first use.

Files:
- `Cargo.toml` — dependencies: serde, serde_json, serde_yaml; optional behind `cli`: server-less, schemars, normalize-output, dirs, jaq-core, jaq-std, jaq-json
- `src/` — library and CLI service source
