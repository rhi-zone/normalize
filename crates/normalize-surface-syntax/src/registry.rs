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
}
