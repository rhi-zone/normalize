# src/ast_grep/lang

Defines the `Lang` type that bridges normalize-languages and ast-grep-core. `Lang` wraps a `DynLang` from `normalize_languages::ast_grep` and implements `ast_grep_core::Language`, providing pattern matching, node metadata, and file-type glob filtering. `lang_globs.rs` maps language names to file glob patterns for directory traversal. This replaces upstream's `SgLang` and eliminates the dependency on `ast-grep-language`'s statically embedded grammars.
