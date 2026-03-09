//! Test-file glob patterns per language, loaded from `data/languages.toml`.
//!
//! Thin wrapper over `LanguageIndex` — all data and caching live there.

use crate::data::LanguageIndex;

/// Return glob patterns that identify dedicated test files for the named language.
///
/// Accepts any reasonable identifier:
/// - Language names (case-insensitive): `"Go"`, `"go"`, `"JavaScript"`
/// - File extensions with or without leading dot: `".go"`, `"rs"`, `".py"`
/// - Common aliases: `"golang"`, `"csharp"`, `"js"`, `"ts"`, `"py"`, `"rb"`
///
/// Returns an empty `Vec` for unknown languages or languages without a dedicated
/// test file naming convention (e.g. C, C++, Zig — which use inline tests).
pub fn test_file_globs_for_language(name: &str) -> Vec<String> {
    LanguageIndex::get().test_globs_for(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_name() {
        assert!(!test_file_globs_for_language("Go").is_empty());
        assert!(!test_file_globs_for_language("Rust").is_empty());
        assert!(!test_file_globs_for_language("JavaScript").is_empty());
        assert!(!test_file_globs_for_language("TypeScript").is_empty());
    }

    #[test]
    fn test_lowercase_name() {
        assert!(!test_file_globs_for_language("go").is_empty());
        assert!(!test_file_globs_for_language("rust").is_empty());
        assert!(!test_file_globs_for_language("javascript").is_empty());
    }

    #[test]
    fn test_extensions_without_dot() {
        assert!(!test_file_globs_for_language("rs").is_empty());
        assert!(!test_file_globs_for_language("py").is_empty());
        assert!(!test_file_globs_for_language("js").is_empty());
        assert!(!test_file_globs_for_language("ts").is_empty());
        assert!(!test_file_globs_for_language("rb").is_empty());
        assert!(!test_file_globs_for_language("kt").is_empty());
    }

    #[test]
    fn test_extensions_with_dot() {
        assert!(!test_file_globs_for_language(".go").is_empty());
        assert!(!test_file_globs_for_language(".rs").is_empty());
        assert!(!test_file_globs_for_language(".py").is_empty());
        assert!(!test_file_globs_for_language(".js").is_empty());
        assert!(!test_file_globs_for_language(".ts").is_empty());
    }

    #[test]
    fn test_aliases() {
        assert!(!test_file_globs_for_language("golang").is_empty());
        assert!(!test_file_globs_for_language("csharp").is_empty());
        assert!(!test_file_globs_for_language("fsharp").is_empty());
        assert!(!test_file_globs_for_language("python").is_empty());
    }

    #[test]
    fn test_unknown_returns_empty() {
        assert!(test_file_globs_for_language("unknown").is_empty());
        assert!(test_file_globs_for_language("").is_empty());
        assert!(test_file_globs_for_language("c").is_empty());
    }

    #[test]
    fn test_data_file_parses_cleanly() {
        // Triggers OnceLock init; panics if TOML is malformed.
        let _ = crate::data::LanguageIndex::get();
    }
}
