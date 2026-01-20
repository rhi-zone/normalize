//! Input format parsers.
//!
//! Each parser reads a schema format and produces an IR [`Schema`](crate::ir::Schema).

pub(crate) mod jsonschema;
mod openapi;

pub use jsonschema::{ParseError, parse_json_schema};
pub use openapi::parse_openapi;
