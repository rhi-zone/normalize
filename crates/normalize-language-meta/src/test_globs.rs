//! Static test-file glob patterns per language name.
//!
//! Returns glob patterns (using `**` wildcards) that identify dedicated test files
//! for a given language. These mirror the `test_file_globs()` implementations in
//! `normalize-languages` but are available as pure static data — no tree-sitter,
//! no registry, no feature flags required.
//!
//! Language names must match `Language::name()` exactly (e.g. "Go", "Rust").

/// Return glob patterns that identify dedicated test files for the named language.
///
/// Returns `&[]` for unknown languages or languages that use only inline tests.
pub fn test_file_globs_for_language(name: &str) -> &'static [&'static str] {
    match name {
        "Clojure" => &["**/*_test.clj", "**/*_test.cljs", "**/*_test.cljc"],
        "C#" => &["**/*Test.cs", "**/*Tests.cs"],
        "Dart" => &["**/test/**/*.dart", "**/*_test.dart"],
        "Elixir" => &["**/test/**/*.exs", "**/*_test.exs"],
        "Erlang" => &["**/*_SUITE.erl", "**/*_test.erl", "**/*_tests.erl"],
        "F#" => &["**/*Test.fs", "**/*Tests.fs"],
        "Go" => &["**/*_test.go"],
        "Groovy" => &[
            "**/src/test/**/*.groovy",
            "**/*Test.groovy",
            "**/*Spec.groovy",
        ],
        "Haskell" => &["**/test/**/*.hs", "**/*Spec.hs", "**/*Test.hs"],
        "Java" => &[
            "**/src/test/**/*.java",
            "**/Test*.java",
            "**/*Test.java",
            "**/*Tests.java",
        ],
        "JavaScript" => &[
            "**/__tests__/**/*.js",
            "**/__mocks__/**/*.js",
            "**/*.test.js",
            "**/*.spec.js",
            "**/*.test.jsx",
            "**/*.spec.jsx",
        ],
        "Kotlin" => &[
            "**/src/test/**/*.kt",
            "**/Test*.kt",
            "**/*Test.kt",
            "**/*Tests.kt",
        ],
        "Perl" => &["**/t/**/*.t", "**/*.t"],
        "PHP" => &["**/*Test.php"],
        "Python" => &["**/test_*.py", "**/*_test.py"],
        "R" => &["**/test-*.R", "**/test_*.R"],
        "Ruby" => &[
            "**/spec/**/*.rb",
            "**/test/**/*.rb",
            "**/*_test.rb",
            "**/*_spec.rb",
        ],
        "Rust" => &["**/tests/**/*.rs"],
        "Scala" => &[
            "**/src/test/**/*.scala",
            "**/*Test.scala",
            "**/*Spec.scala",
            "**/*Suite.scala",
        ],
        "Swift" => &["**/*Tests.swift", "**/*Test.swift"],
        "TypeScript" | "TSX" => &[
            "**/__tests__/**/*.ts",
            "**/__mocks__/**/*.ts",
            "**/*.test.ts",
            "**/*.spec.ts",
            "**/*.test.tsx",
            "**/*.spec.tsx",
        ],
        "Visual Basic" => &["**/*Test.vb", "**/*Tests.vb"],
        _ => &[],
    }
}
