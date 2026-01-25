//! Input format parsers.
//!
//! Each parser reads a schema format and produces an IR [`Schema`](crate::ir::Schema).

pub(crate) mod jsonschema;
mod openapi;
#[cfg(feature = "input-typescript")]
pub mod typescript;

pub use jsonschema::{ParseError, parse_json_schema};
pub use openapi::parse_openapi;
#[cfg(feature = "input-typescript")]
pub use typescript::parse_typescript_types;
