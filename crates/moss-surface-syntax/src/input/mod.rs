//! Input readers - parse source code into IR.

#[cfg(feature = "read-typescript")]
pub mod typescript;

#[cfg(feature = "read-lua")]
pub mod lua;

#[cfg(feature = "read-typescript")]
pub use typescript::{ReadError, read_typescript};
