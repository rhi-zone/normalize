Source for normalize-knowledge-graph crate.

- `lib.rs` — crate root, module declarations
- `model.rs` — Unit (with `links: Vec<Link>`), Link, Edge types; ID validation (`[a-z0-9][a-z0-9-]*`); dotted-path metadata lookup
- `store.rs` — filesystem I/O: unit CRUD (YAML frontmatter + markdown body), per-unit link operations (`link`/`unlink`/`list_all_edges`), legacy `edges.jsonl` migration
- `query.rs` — predicate matching (dotted-path, string equality), edge filtering, BFS neighbor traversal
- `reports.rs` — UnitReport, DeleteReport, EdgeReport, EdgeListReport, QueryReport, NeighborsReport, ShowReport + OutputFormatter impls (behind `cli` feature)
- `service.rs` — KgCliService with #[cli] annotations for all `normalize kg` subcommands (behind `cli` feature)
