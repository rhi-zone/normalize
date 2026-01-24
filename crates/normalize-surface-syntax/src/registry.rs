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
    READERS.write().unwrap().push(reader);
}

/// Register a custom writer.
pub fn register_writer(writer: &'static dyn Writer) {
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
    });
}

/// Get a reader by language name.
pub fn reader_for_language(lang: &str) -> Option<&'static dyn Reader> {
    init_readers();
    READERS
        .read()
        .unwrap()
        .iter()
        .find(|r| r.language() == lang)
        .copied()
}

/// Get a reader by file extension.
pub fn reader_for_extension(ext: &str) -> Option<&'static dyn Reader> {
    init_readers();
    READERS
        .read()
        .unwrap()
        .iter()
        .find(|r| r.extensions().contains(&ext))
        .copied()
}

/// Get a writer by language name.
pub fn writer_for_language(lang: &str) -> Option<&'static dyn Writer> {
    init_writers();
    WRITERS
        .read()
        .unwrap()
        .iter()
        .find(|w| w.language() == lang)
        .copied()
}

/// Get all registered readers.
pub fn readers() -> Vec<&'static dyn Reader> {
    init_readers();
    READERS.read().unwrap().clone()
}

/// Get all registered writers.
pub fn writers() -> Vec<&'static dyn Writer> {
    init_writers();
    WRITERS.read().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StructureEq;

    #[test]
    #[cfg(feature = "read-typescript")]
    fn test_reader_lookup() {
        let reader = reader_for_language("typescript").expect("typescript reader");
        assert_eq!(reader.language(), "typescript");
        assert!(reader.extensions().contains(&"ts"));

        let reader = reader_for_extension("tsx").expect("tsx extension");
        assert_eq!(reader.language(), "typescript");
    }

    #[test]
    #[cfg(feature = "write-lua")]
    fn test_writer_lookup() {
        let writer = writer_for_language("lua").expect("lua writer");
        assert_eq!(writer.language(), "lua");
        assert_eq!(writer.extension(), "lua");
    }

    #[test]
    #[cfg(all(feature = "read-typescript", feature = "write-lua"))]
    fn test_roundtrip_via_registry() {
        let reader = reader_for_language("typescript").unwrap();
        let writer = writer_for_language("lua").unwrap();

        let ir = reader.read("const x = 1 + 2;").unwrap();
        let lua = writer.write(&ir);

        assert!(lua.contains("local x"));
    }

    #[test]
    #[cfg(feature = "read-lua")]
    fn test_lua_reader_lookup() {
        let reader = reader_for_language("lua").expect("lua reader");
        assert_eq!(reader.language(), "lua");
        assert!(reader.extensions().contains(&"lua"));
    }

    #[test]
    #[cfg(feature = "write-typescript")]
    fn test_typescript_writer_lookup() {
        let writer = writer_for_language("typescript").expect("typescript writer");
        assert_eq!(writer.language(), "typescript");
        assert_eq!(writer.extension(), "ts");
    }

    #[test]
    #[cfg(all(feature = "read-lua", feature = "write-typescript"))]
    fn test_lua_to_typescript_roundtrip() {
        let reader = reader_for_language("lua").unwrap();
        let writer = writer_for_language("typescript").unwrap();

        let ir = reader.read("local x = 1 + 2").unwrap();
        let ts = writer.write(&ir);

        assert!(ts.contains("let x") || ts.contains("const x"));
        assert!(ts.contains("1 + 2") || ts.contains("(1 + 2)"));
    }

    #[test]
    #[cfg(all(feature = "read-typescript", feature = "write-typescript"))]
    fn test_typescript_roundtrip() {
        let reader = reader_for_language("typescript").unwrap();
        let writer = writer_for_language("typescript").unwrap();

        let ir = reader.read("const x = 1 + 2;").unwrap();
        let ts = writer.write(&ir);

        assert!(ts.contains("const x"));
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
    fn test_structure_eq_ts_lua_variable() {
        let ts_reader = reader_for_language("typescript").unwrap();
        let lua_writer = writer_for_language("lua").unwrap();
        let lua_reader = reader_for_language("lua").unwrap();

        // TS → IR₁
        let ir1 = ts_reader.read("const x = 42;").unwrap();
        // IR₁ → Lua
        let lua = lua_writer.write(&ir1);
        // Lua → IR₂
        let ir2 = lua_reader.read(&lua).unwrap();

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
    }

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_binary_expr() {
        let ts_reader = reader_for_language("typescript").unwrap();
        let lua_writer = writer_for_language("lua").unwrap();
        let lua_reader = reader_for_language("lua").unwrap();

        let ir1 = ts_reader.read("let result = 1 + 2 * 3;").unwrap();
        let lua = lua_writer.write(&ir1);
        let ir2 = lua_reader.read(&lua).unwrap();

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
    }

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_function_call() {
        let ts_reader = reader_for_language("typescript").unwrap();
        let lua_writer = writer_for_language("lua").unwrap();
        let lua_reader = reader_for_language("lua").unwrap();

        let ir1 = ts_reader.read("console.log(\"hello\", 42);").unwrap();
        let lua = lua_writer.write(&ir1);
        let ir2 = lua_reader.read(&lua).unwrap();

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
    }

    #[test]
    #[cfg(all(
        feature = "read-typescript",
        feature = "write-lua",
        feature = "read-lua"
    ))]
    fn test_structure_eq_ts_lua_if_statement() {
        let ts_reader = reader_for_language("typescript").unwrap();
        let lua_writer = writer_for_language("lua").unwrap();
        let lua_reader = reader_for_language("lua").unwrap();

        let ir1 = ts_reader.read("if (x > 0) { console.log(x); }").unwrap();
        let lua = lua_writer.write(&ir1);
        let ir2 = lua_reader.read(&lua).unwrap();

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nLua: {}\nIR₂: {:?}",
            ir1,
            lua,
            ir2
        );
    }

    #[test]
    #[cfg(all(
        feature = "read-lua",
        feature = "write-typescript",
        feature = "read-typescript"
    ))]
    fn test_structure_eq_lua_ts_variable() {
        let lua_reader = reader_for_language("lua").unwrap();
        let ts_writer = writer_for_language("typescript").unwrap();
        let ts_reader = reader_for_language("typescript").unwrap();

        // Lua → IR₁
        let ir1 = lua_reader.read("local x = 42").unwrap();
        // IR₁ → TS
        let ts = ts_writer.write(&ir1);
        // TS → IR₂
        let ir2 = ts_reader.read(&ts).unwrap();

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nTS: {}\nIR₂: {:?}",
            ir1,
            ts,
            ir2
        );
    }

    #[test]
    #[cfg(all(
        feature = "read-lua",
        feature = "write-typescript",
        feature = "read-typescript"
    ))]
    fn test_structure_eq_lua_ts_function() {
        let lua_reader = reader_for_language("lua").unwrap();
        let ts_writer = writer_for_language("typescript").unwrap();
        let ts_reader = reader_for_language("typescript").unwrap();

        let ir1 = lua_reader
            .read("function add(a, b) return a + b end")
            .unwrap();
        let ts = ts_writer.write(&ir1);
        let ir2 = ts_reader.read(&ts).unwrap();

        assert!(
            ir1.structure_eq(&ir2),
            "IR mismatch:\nIR₁: {:?}\nTS: {}\nIR₂: {:?}",
            ir1,
            ts,
            ir2
        );
    }
}
