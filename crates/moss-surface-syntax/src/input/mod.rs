//! Input readers - parse source code into IR.

#[cfg(feature = "read-typescript")]
pub mod typescript;

#[cfg(feature = "read-typescript")]
pub use typescript::{TYPESCRIPT_READER, TypeScriptReader, read_typescript};
