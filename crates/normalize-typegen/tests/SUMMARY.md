# normalize-typegen/tests

Snapshot and fixture tests for type generation backends.

`codegen.rs` exercises all backends (TypeScript, Zod, Valibot, Pydantic, Python dataclasses/TypedDict, Go, Rust) against JSON fixture schemas from `fixtures/` and tagged-union inputs, comparing output to insta snapshots in `snapshots/`. Covers structs, string enums, optional fields, read-only types, frozen models, no-serde Rust, and OpenAPI (Petstore) round-trips.
