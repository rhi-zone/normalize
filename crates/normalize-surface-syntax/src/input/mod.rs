//! Input readers - parse source code into IR.

#[cfg(feature = "read-typescript")]
pub mod typescript;

#[cfg(feature = "read-typescript")]
pub use typescript::{TYPESCRIPT_READER, TypeScriptReader, read_typescript};

#[cfg(feature = "read-lua")]
pub mod lua;

#[cfg(feature = "read-lua")]
pub use lua::{LUA_READER, LuaReader, read_lua};

#[cfg(feature = "read-python")]
pub mod python;

#[cfg(feature = "read-python")]
pub use python::{PYTHON_READER, PythonReader, read_python};
