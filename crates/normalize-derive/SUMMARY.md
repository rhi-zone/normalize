# normalize-derive

Procedural derive macros for the normalize ecosystem.

A `proc-macro` crate providing `#[derive(Merge)]`, which generates a `normalize_core::Merge` implementation for named-field and tuple structs by calling `.merge()` on each field. Enums and unions are rejected at compile time. Re-exported from `normalize-core` so downstream crates only need to depend on `normalize-core`.
