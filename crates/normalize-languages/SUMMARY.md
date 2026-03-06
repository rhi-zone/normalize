# normalize-languages

Tree-sitter language support for ~98 programming languages.

Each language is a zero-sized struct (e.g., `Python`, `Rust`, `Go`, `TypeScript`) implementing the `Language` trait, which provides symbol extraction, import parsing, visibility detection, docstring extraction, test file globs, and embedded content support. Grammars are loaded dynamically from compiled shared libraries via `GrammarLoader` (backed by libloading), with query files (`.scm`) loaded from `src/queries/`. The crate also provides `support_for_path`, `support_for_extension`, `support_for_grammar`, and `supported_languages` registry functions. All 98 languages are individually feature-gated under `lang-*` flags grouped into `langs-core`, `langs-functional`, `langs-config`, `langs-data`, `langs-markup`, `langs-hardware`, and `langs-misc`.
