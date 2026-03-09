//! Static test-file glob patterns per language name.
//!
//! Returns glob patterns (using `**` wildcards) that identify dedicated test files
//! for a given language. These mirror the `test_file_globs()` implementations in
//! `normalize-languages` but are available as pure static data — no tree-sitter,
//! no registry, no feature flags required.

/// Return glob patterns that identify dedicated test files for the named language.
///
/// Accepts any reasonable identifier for a language:
/// - Language names (case-insensitive): `"Go"`, `"go"`, `"JavaScript"`
/// - File extensions with or without leading dot: `".go"`, `"go"`, `".rs"`, `"rs"`
/// - Common aliases: `"golang"`, `"js"`, `"ts"`, `"py"`, `"rb"`, `"csharp"`, `"fsharp"`
///
/// Returns `&[]` for unknown languages or languages without a dedicated test file convention.
pub fn test_file_globs_for_language(name: &str) -> &'static [&'static str] {
    let name = name.strip_prefix('.').unwrap_or(name);
    match &*name.to_ascii_lowercase() {
        "clojure" | "clj" | "cljs" | "cljc" => {
            &["**/*_test.clj", "**/*_test.cljs", "**/*_test.cljc"]
        }
        "c#" | "cs" | "csharp" => &["**/*Test.cs", "**/*Tests.cs"],
        "dart" => &["**/test/**/*.dart", "**/*_test.dart"],
        "elixir" | "ex" | "exs" => &["**/test/**/*.exs", "**/*_test.exs"],
        "erlang" | "erl" | "hrl" => &["**/*_SUITE.erl", "**/*_test.erl", "**/*_tests.erl"],
        "f#" | "fs" | "fsi" | "fsx" | "fsharp" => &["**/*Test.fs", "**/*Tests.fs"],
        "go" | "golang" => &["**/*_test.go"],
        "groovy" | "gvy" => &[
            "**/src/test/**/*.groovy",
            "**/*Test.groovy",
            "**/*Spec.groovy",
        ],
        "haskell" | "hs" | "lhs" => &["**/test/**/*.hs", "**/*Spec.hs", "**/*Test.hs"],
        "java" => &[
            "**/src/test/**/*.java",
            "**/Test*.java",
            "**/*Test.java",
            "**/*Tests.java",
        ],
        "javascript" | "js" | "jsx" => &[
            "**/__tests__/**/*.js",
            "**/__mocks__/**/*.js",
            "**/*.test.js",
            "**/*.spec.js",
            "**/*.test.jsx",
            "**/*.spec.jsx",
        ],
        "kotlin" | "kt" | "kts" => &[
            "**/src/test/**/*.kt",
            "**/Test*.kt",
            "**/*Test.kt",
            "**/*Tests.kt",
        ],
        "perl" | "pl" | "pm" => &["**/t/**/*.t", "**/*.t"],
        "php" | "php3" | "php4" | "php5" => &["**/*Test.php"],
        "python" | "py" | "pyw" => &["**/test_*.py", "**/*_test.py"],
        "r" | "rscript" => &["**/test-*.R", "**/test_*.R"],
        "ruby" | "rb" => &[
            "**/spec/**/*.rb",
            "**/test/**/*.rb",
            "**/*_test.rb",
            "**/*_spec.rb",
        ],
        "rust" | "rs" => &["**/tests/**/*.rs"],
        "scala" | "sc" => &[
            "**/src/test/**/*.scala",
            "**/*Test.scala",
            "**/*Spec.scala",
            "**/*Suite.scala",
        ],
        "swift" => &["**/*Tests.swift", "**/*Test.swift"],
        "typescript" | "ts" | "tsx" => &[
            "**/__tests__/**/*.ts",
            "**/__mocks__/**/*.ts",
            "**/*.test.ts",
            "**/*.spec.ts",
            "**/*.test.tsx",
            "**/*.spec.tsx",
        ],
        "visual basic" | "vb" => &["**/*Test.vb", "**/*Tests.vb"],
        _ => &[],
    }
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
        assert!(!test_file_globs_for_language("go").is_empty());
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
    }

    #[test]
    fn test_unknown_returns_empty() {
        assert!(test_file_globs_for_language("unknown").is_empty());
        assert!(test_file_globs_for_language("").is_empty());
        assert!(test_file_globs_for_language("c").is_empty());
    }
}
