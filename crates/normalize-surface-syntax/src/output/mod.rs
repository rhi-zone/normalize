//! Output writers - emit IR as source code.

#[cfg(feature = "write-lua")]
pub mod lua;

#[cfg(feature = "write-lua")]
pub use lua::{LUA_WRITER, LuaWriter, LuaWriterImpl};

#[cfg(feature = "write-typescript")]
pub mod typescript;

#[cfg(feature = "write-typescript")]
pub use typescript::{TYPESCRIPT_WRITER, TypeScriptWriter, TypeScriptWriterImpl};

#[cfg(feature = "write-python")]
pub mod python;

#[cfg(feature = "write-python")]
pub use python::{PYTHON_WRITER, PythonWriter, PythonWriterImpl};
