//! Input format parsers.
//!
//! Each parser reads a schema format and produces an IR [`Schema`](crate::ir::Schema).

#[cfg(feature = "input-graphql")]
pub mod graphql;
pub(crate) mod jsonschema;
mod openapi;
pub mod proto;
#[cfg(feature = "input-typescript")]
pub mod typescript;

#[cfg(feature = "input-graphql")]
pub use graphql::parse_graphql_schema;
pub use jsonschema::{ParseError, parse_json_schema};
pub use openapi::parse_openapi;
pub use proto::parse_proto;
#[cfg(feature = "input-typescript")]
pub use typescript::parse_typescript_types;
