//! Output writers - emit IR as source code.

#[cfg(feature = "write-lua")]
pub mod lua;

#[cfg(feature = "write-lua")]
pub use lua::{LUA_WRITER, LuaWriter, LuaWriterImpl};
