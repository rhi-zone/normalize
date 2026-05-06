# normalize-typegen

Polyglot type and validator generation from schemas.

Converts JSON Schema, OpenAPI, Protobuf IDL, GraphQL SDL, and TypeScript inputs into a common `Schema` IR (`TypeDef`, `Field`, `Type`, `EnumKind`, `TaggedUnion`), then generates idiomatic output for multiple backends: TypeScript interfaces, Zod schemas, Valibot schemas, Python dataclasses/TypedDict, Pydantic models, Go structs with json tags, and Rust serde structs. Backends implement the `Backend` trait and are registered globally via `register_backend`; `get_backend("zod")` / `backends_for_language("typescript")` provide lookup. All backends are feature-gated under `backend-*` flags. Input parsers: JSON Schema and OpenAPI are always available; TypeScript requires `input-typescript`; GraphQL requires `input-graphql`; Protobuf has no feature gate.
