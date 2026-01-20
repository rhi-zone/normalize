//! Input format parsers.
//!
//! Each parser reads a schema format and produces an IR [`Schema`](crate::ir::Schema).

mod jsonschema;

pub use jsonschema::parse_json_schema;
