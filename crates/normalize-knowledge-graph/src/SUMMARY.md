Source for normalize-knowledge-graph crate.

- `lib.rs` — crate root, module declarations
- `model.rs` — Unit (with `links: Vec<Link>`), Link, Edge types; ID validation (`[a-z0-9][a-z0-9-]*`); dotted-path metadata lookup
- `store.rs` — filesystem I/O: unit CRUD, jq transform/predicate eval via jaq, BFS walk_from, legacy `edges.jsonl` migration (all behind `cli` feature for jq functions)
- `reports.rs` — UnitReport, ReadReport, WriteReport, WalkReport + OutputFormatter impls (behind `cli` feature)
- `service.rs` — KgCliService with #[cli] annotations for `read`, `write`, `walk` (behind `cli` feature)
