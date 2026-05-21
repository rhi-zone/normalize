Persistent knowledge graph adjacent to code. Provides unit CRUD (create/get/set/append/delete), edge management (link/unlink stored in per-unit frontmatter `links` arrays), BFS traversal (neighbors), and metadata query. Exposed via `normalize kg` subcommands. Storage in `.normalize/kg/` — unit files as `<id>.md` with YAML frontmatter; outgoing edges in each unit's `links` field. Auto-migrates legacy `edges.jsonl` logs on first use.

Files:
- `Cargo.toml` — dependencies: serde, serde_json, serde_yaml, chrono, dirs; optional: server-less, schemars, normalize-output (behind `cli` feature)
- `src/` — library and CLI service source
