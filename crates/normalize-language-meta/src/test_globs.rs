//! Static test-file glob patterns per language name.
//!
//! Returns glob patterns (using `**` wildcards) that identify dedicated test files
//! for a given language. These mirror the `test_file_globs()` implementations in
//! `normalize-languages` but are available as pure static data — no tree-sitter,
//! no registry, no feature flags required.

/// Return glob patterns that identify dedicated test files for the named language.
///
/// Matching is case-insensitive: "go", "Go", and "GO" all work.
/// Returns `&[]` for unknown languages or languages that use only inline tests.
pub fn test_file_globs_for_language(name: &str) -> &'static [&'static str] {
    // Case-insensitive: compare lowercased input against lowercased canonical names.
    // This table maps lowercase canonical names → globs.
    match &*name.to_ascii_lowercase() {
        "clojure" => &["**/*_test.clj", "**/*_test.cljs", "**/*_test.cljc"],
        "c#" => &["**/*Test.cs", "**/*Tests.cs"],
        "dart" => &["**/test/**/*.dart", "**/*_test.dart"],
        "elixir" => &["**/test/**/*.exs", "**/*_test.exs"],
        "erlang" => &["**/*_SUITE.erl", "**/*_test.erl", "**/*_tests.erl"],
        "f#" => &["**/*Test.fs", "**/*Tests.fs"],
        "go" => &["**/*_test.go"],
        "groovy" => &[
            "**/src/test/**/*.groovy",
            "**/*Test.groovy",
            "**/*Spec.groovy",
        ],
        "haskell" => &["**/test/**/*.hs", "**/*Spec.hs", "**/*Test.hs"],
        "java" => &[
            "**/src/test/**/*.java",
            "**/Test*.java",
            "**/*Test.java",
            "**/*Tests.java",
        ],
        "javascript" => &[
            "**/__tests__/**/*.js",
            "**/__mocks__/**/*.js",
            "**/*.test.js",
            "**/*.spec.js",
            "**/*.test.jsx",
            "**/*.spec.jsx",
        ],
        "kotlin" => &[
            "**/src/test/**/*.kt",
            "**/Test*.kt",
            "**/*Test.kt",
            "**/*Tests.kt",
        ],
        "perl" => &["**/t/**/*.t", "**/*.t"],
        "php" => &["**/*Test.php"],
        "python" => &["**/test_*.py", "**/*_test.py"],
        "r" => &["**/test-*.R", "**/test_*.R"],
        "ruby" => &[
            "**/spec/**/*.rb",
            "**/test/**/*.rb",
            "**/*_test.rb",
            "**/*_spec.rb",
        ],
        "rust" => &["**/tests/**/*.rs"],
        "scala" => &[
            "**/src/test/**/*.scala",
            "**/*Test.scala",
            "**/*Spec.scala",
            "**/*Suite.scala",
        ],
        "swift" => &["**/*Tests.swift", "**/*Test.swift"],
        "typescript" | "tsx" => &[
            "**/__tests__/**/*.ts",
            "**/__mocks__/**/*.ts",
            "**/*.test.ts",
            "**/*.spec.ts",
            "**/*.test.tsx",
            "**/*.spec.tsx",
        ],
        "visual basic" => &["**/*Test.vb", "**/*Tests.vb"],
        _ => &[],
    }
}
