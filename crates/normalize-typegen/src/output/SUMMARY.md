# normalize-typegen/src/output

Code generation backends for the typegen IR.

Each file implements the `Backend` trait for one target: `typescript.rs` (TypeScript interfaces/types), `zod.rs` (Zod runtime schemas), `valibot.rs` (Valibot schemas), `python.rs` (dataclasses and TypedDict), `pydantic.rs` (Pydantic v2 models), `go.rs` (Go structs with json struct tags), `rust.rs` (Rust structs with serde derives). All are feature-gated under the corresponding `backend-*` flag. `mod.rs` re-exports top-level `generate_*` convenience functions.
