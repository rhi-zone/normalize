# normalize-syntax-rules

Syntax-based linting with tree-sitter queries, providing built-in rules and a rule loading/execution engine. The standalone CLI binary (enabled with `cli` feature) exposes `RunRulesReport` and `RulesListReport` output types that implement `OutputFormatter`.

Rules are `.scm` files with a TOML frontmatter header (id, severity, message, allow globs, requires predicates, optional fix template) followed by a tree-sitter or ast-grep S-expression pattern. Key types: `Rule`, `Finding`, `Severity`, `RulesConfig`, `RuleOverride`, `BuiltinRule`. Key functions: `load_all_rules()`, `run_rules()`, `apply_fixes()`, `expand_fix_template()`. Rules are loaded from three sources in priority order: embedded builtins, user global (`~/.config/normalize/rules/`), and project-local (`.normalize/rules/`).

Includes 94 builtin syntax rules across Rust (13), JavaScript (10), TypeScript (5), Python (12), Go (9), C/C++ (4), Java (6), C# (6), Kotlin (5), Swift (5), PHP (5), Ruby (9), and cross-language (4). Covers debug prints, security patterns, style issues, idiomatic patterns, error handling, and code quality. The `SourceRegistry` / `RuleSource` trait system provides namespace-keyed data (path, env, git, language-specific) used to evaluate `requires` predicates. Fixture tests in `tests/` support `match.*`, `no_match.*`, and `fix.*`/`fix.expected.*` files for testing detection and auto-fix transforms.

`run_rules()` takes a separate `project_root: &Path` parameter (for allow-list matching) and `root` (scan target), enabling correct path matching when scanning a subdirectory. `apply_fixes()` loops until no fixable findings remain, handling nested fixes.
