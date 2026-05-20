Persistent knowledge graph adjacent to code. Provides unit CRUD (create/get/set/append/delete), edge management (link/unlink with append-only log), BFS traversal (neighbors), and metadata query. Exposed via `normalize kg` subcommands. Storage in `.normalize/kg/` — unit files as `<id>.md` with YAML frontmatter, edge log as `edges.jsonl`.

Files:
- `Cargo.toml` — dependencies: serde, serde_json, serde_yaml, chrono, dirs; optional: server-less, schemars, normalize-output (behind `cli` feature)
- `src/` — library and CLI service source
