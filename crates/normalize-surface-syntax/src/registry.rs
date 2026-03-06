//! Registry for readers and writers.

use crate::traits::{Reader, Writer};
use std::sync::{OnceLock, RwLock};

/// Global reader registry.
static READERS: RwLock<Vec<&'static dyn Reader>> = RwLock::new(Vec::new());
static READERS_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Global writer registry.
static WRITERS: RwLock<Vec<&'static dyn Writer>> = RwLock::new(Vec::new());
static WRITERS_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a custom reader.
pub fn register_reader(reader: &'static dyn Reader) {
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    READERS.write().unwrap().push(reader);
}

/// Register a custom writer.
pub fn register_writer(writer: &'static dyn Writer) {
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    WRITERS.write().unwrap().push(writer);
}

fn init_readers() {
    READERS_INITIALIZED.get_or_init(|| {
        #[cfg(feature = "read-typescript")]
        {
            register_reader(&crate::input::typescript::TYPESCRIPT_READER);
        }
        #[cfg(feature = "read-lua")]
        {
            register_reader(&crate::input::lua::LUA_READER);
        }
        #[cfg(feature = "read-python")]
        {
            register_reader(&crate::input::python::PYTHON_READER);
        }
    });
}

fn init_writers() {
    WRITERS_INITIALIZED.get_or_init(|| {
        #[cfg(feature = "write-lua")]
        {
            register_writer(&crate::output::lua::LUA_WRITER);
        }
        #[cfg(feature = "write-typescript")]
        {
            register_writer(&crate::output::typescript::TYPESCRIPT_WRITER);
        }
        #[cfg(feature = "write-python")]
        {
            register_writer(&crate::output::python::PYTHON_WRITER);
        }
    });
}

/// Get a reader by language name.
pub fn reader_for_language(lang: &str) -> Option<&'static dyn Reader> {
    init_readers();
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    let guard = READERS.read().unwrap();
    guard.iter().find(|r| r.language() == lang).copied()
}

/// Get a reader by file extension.
pub fn reader_for_extension(ext: &str) -> Option<&'static dyn Reader> {
    init_readers();
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    let guard = READERS.read().unwrap();
    guard
        .iter()
        .find(|r| r.extensions().contains(&ext))
        .copied()
}

/// Get a writer by language name.
pub fn writer_for_language(lang: &str) -> Option<&'static dyn Writer> {
    init_writers();
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    let guard = WRITERS.read().unwrap();
    guard.iter().find(|w| w.language() == lang).copied()
}

/// Get all registered readers.
pub fn readers() -> Vec<&'static dyn Reader> {
    init_readers();
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    READERS.read().unwrap().clone()
}

/// Get all registered writers.
pub fn writers() -> Vec<&'static dyn Writer> {
    init_writers();
    // normalize-syntax-allow: rust/unwrap-in-impl - static RwLock, poison only on programmer error
    WRITERS.read().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StructureEq;

    #[test]
    #[cfg(feature = "read-typescript")]
    fn test_reader_lookup() -> Result<(), String> {
        let reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        assert_eq!(reader.language(), "typescript");
        assert!(reader.extensions().contains(&"ts"));

        let reader = reader_for_extension("tsx").ok_or("tsx extension not found")?;
        assert_eq!(reader.language(), "typescript");
        Ok(())
    }

    #[test]
    #[cfg(feature = "write-lua")]
    fn test_writer_lookup() -> Result<(), String> {
        let writer = writer_for_language("lua").ok_or("lua writer not found")?;
        assert_eq!(writer.language(), "lua");
        assert_eq!(writer.extension(), "lua");
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "read-typescript", feature = "write-lua"))]
    fn test_roundtrip_via_registry() -> Result<(), String> {
        let reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        let writer = writer_for_language("lua").ok_or("lua writer not found")?;

        let ir = reader.read("const x = 1 + 2;").map_err(|e| e.to_string())?;
        let lua = writer.write(&ir);

        assert!(lua.contains("local x"));
        Ok(())
    }

    #[test]
    #[cfg(feature = "read-lua")]
    fn test_lua_reader_lookup() -> Result<(), String> {
        let reader = reader_for_language("lua").ok_or("lua reader not found")?;
        assert_eq!(reader.language(), "lua");
        assert!(reader.extensions().contains(&"lua"));
        Ok(())
    }

    #[test]
    #[cfg(feature = "write-typescript")]
    fn test_typescript_writer_lookup() -> Result<(), String> {
        let writer = writer_for_language("typescript").ok_or("typescript writer not found")?;
        assert_eq!(writer.language(), "typescript");
        assert_eq!(writer.extension(), "ts");
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "read-lua", feature = "write-typescript"))]
    fn test_lua_to_typescript_roundtrip() -> Result<(), String> {
        let reader = reader_for_language("lua").ok_or("lua reader not found")?;
        let writer = writer_for_language("typescript").ok_or("typescript writer not found")?;

        let ir = reader.read("local x = 1 + 2").map_err(|e| e.to_string())?;
        let ts = writer.write(&ir);

        assert!(ts.contains("let x") || ts.contains("const x"));
        assert!(ts.contains("1 + 2") || ts.contains("(1 + 2)"));
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "read-typescript", feature = "write-typescript"))]
    fn test_typescript_roundtrip() -> Result<(), String> {
        let reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        let writer = writer_for_language("typescript").ok_or("typescript writer not found")?;

        let ir = reader.read("const x = 1 + 2;").map_err(|e| e.to_string())?;
        let ts = writer.write(&ir);

        assert!(ts.contains("const x"));
        Ok(())
    }

    // ========================================================================
    // Roundtrip tests with structure_eq
    // ========================================================================
    //
    // These tests verify that IR is preserved through:
    //   Source₁ → IR₁ → Source₂ → IR₂
    // Using structure_eq to ignore surface hints (mutable, computed, etc.)

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_variable() -> Result<(), String> {
        let ts_reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        let lua_writer = writer_for_language("lua").ok_or("lua writer not found")?;
        let lua_reader = reader_for_language("lua").ok_or("lua reader not found")?;

        // TS → IR₁
        let ir1 = ts_reader.read("const x = 42;").map_err(|e| e.to_string())?;
        // IR₁ → Lua
        let lua = lua_writer.write(&ir1);
        // Lua → IR₂
        let ir2 = lua_reader.read(&lua).map_err(|e| e.to_string())?;

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
        Ok(())
    }

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_binary_expr() -> Result<(), String> {
        let ts_reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        let lua_writer = writer_for_language("lua").ok_or("lua writer not found")?;
        let lua_reader = reader_for_language("lua").ok_or("lua reader not found")?;

        let ir1 = ts_reader
            .read("let result = 1 + 2 * 3;")
            .map_err(|e| e.to_string())?;
        let lua = lua_writer.write(&ir1);
        let ir2 = lua_reader.read(&lua).map_err(|e| e.to_string())?;

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
        Ok(())
    }

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_function_call() -> Result<(), String> {
        let ts_reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        let lua_writer = writer_for_language("lua").ok_or("lua writer not found")?;
        let lua_reader = reader_for_language("lua").ok_or("lua reader not found")?;

        let ir1 = ts_reader
            .read("console.log(\"hello\", 42);")
            .map_err(|e| e.to_string())?;
        let lua = lua_writer.write(&ir1);
        let ir2 = lua_reader.read(&lua).map_err(|e| e.to_string())?;

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
        Ok(())
    }

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_if_statement() -> Result<(), String> {
        let ts_reader = reader_for_language("typescript").ok_or("typescript reader not found")?;
        let lua_writer = writer_for_language("lua").ok_or("lua writer not found")?;
        let lua_reader = reader_for_language("lua").ok_or("lua reader not found")?;

        let ir1 = ts_reader
            .read("if (x > 0) { console.log(x); }")
            .map_err(|e| e.to_string())?;
        let lua = lua_writer.write(&ir1);
        let ir2 = lua_reader.read(&lua).map_err(|e| e.to_string())?;

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
        Ok(())
    }

    #[test]
    #[cfg(all(
        feature = "read-lua",
        feature = "write-typescript",
        feature = "read-typescript"
    ))]
    fn test_structure_eq_lua_ts_variable() -> Result<(), String> {
        let lua_reader = reader_for_language("lua").ok_or("lua reader not found")?;
        let ts_writer = writer_for_language("typescript").ok_or("typescript writer not found")?;
        let ts_reader = reader_for_language("typescript").ok_or("typescript reader not found")?;

        // Lua → IR₁
        let ir1 = lua_reader.read("local x = 42").map_err(|e| e.to_string())?;
        // IR₁ → TS
        let ts = ts_writer.write(&ir1);
        // TS → IR₂
        let ir2 = ts_reader.read(&ts).map_err(|e| e.to_string())?;

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nTS: {}\nIR₂: {:?}",
            ir1,
            ts,
            ir2
        );
        Ok(())
    }

    #[test]
    #[cfg(all(
        feature = "read-lua",
        feature = "write-typescript",
        feature = "read-typescript"
    ))]
    fn test_structure_eq_lua_ts_function() -> Result<(), String> {
        let lua_reader = reader_for_language("lua").ok_or("lua reader not found")?;
        let ts_writer = writer_for_language("typescript").ok_or("typescript writer not found")?;
        let ts_reader = reader_for_language("typescript").ok_or("typescript reader not found")?;

        let ir1 = lua_reader
            .read("function add(a, b) return a + b end")
            .map_err(|e| e.to_string())?;
        let ts = ts_writer.write(&ir1);
        let ir2 = ts_reader.read(&ts).map_err(|e| e.to_string())?;

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nTS: {}\nIR₂: {:?}",
            ir1,
            ts,
            ir2
        );
        Ok(())
    }
}
