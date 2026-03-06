# src/ast_grep

Vendored and adapted embedding of ast-grep 0.41.0 (MIT) as a drop-in `ast-grep`/`sg` replacement. The key modification is replacing ast-grep's built-in `SgLang` type with a `Lang` wrapper that uses normalize-languages' `GrammarLoader` for dynamic tree-sitter grammar loading, avoiding grammar duplication. Submodules: `lang` (language type + glob registry), `print` (output formatters), `utils` (shared CLI arg types, worker threading, debug utilities), `verify` (rule test harness), `run` (single-pattern search), `scan` (config-driven multi-rule scan).
