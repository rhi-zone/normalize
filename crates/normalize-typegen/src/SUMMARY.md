# normalize-typegen/src

Source for the polyglot type-generation crate.

Key modules: `ir.rs` (the shared `Schema`, `TypeDef`, `TypeDefKind`, `Field`, `Type`, `EnumDef`, `TaggedUnion` IR), `input/` (parsers for JSON Schema, OpenAPI, and optionally TypeScript), `output/` (code generation backends — TypeScript, Zod, Valibot, Python, Pydantic, Go, Rust), `traits.rs` (`Backend` trait + `BackendCategory`), `registry.rs` (global backend registry with `get_backend`, `backends_for_language`, `backend_names`).
