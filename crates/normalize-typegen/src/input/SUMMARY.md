# normalize-typegen/src/input

Input format parsers that produce the typegen IR.

`jsonschema.rs` parses JSON Schema objects (via `serde_json::Value`) into a `Schema`; exports `parse_json_schema` and `ParseError`. `openapi.rs` parses OpenAPI 3.x documents (components/schemas section) into the same IR via `parse_openapi`. `typescript.rs` (feature `input-typescript`) parses TypeScript type declarations using tree-sitter via arborium-typescript; exports `parse_typescript_types`. All parsers return `Result<Schema, ParseError>`.
