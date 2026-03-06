# normalize-syntax-rules

Syntax-based linting with tree-sitter queries, providing built-in rules and a rule loading/execution engine.

Rules are `.scm` files with a TOML frontmatter header (id, severity, message, allow globs, requires predicates, fix template) followed by a tree-sitter or ast-grep S-expression pattern. Key types: `Rule`, `Finding`, `Severity`, `RulesConfig`, `RuleOverride`, `BuiltinRule`. Key functions: `load_all_rules()`, `run_rules()`, `apply_fixes()`. Rules are loaded from three sources in priority order: embedded builtins, user global (`~/.config/normalize/rules/`), and project-local (`.normalize/rules/`).

Includes 29 builtin rules across Rust, JavaScript/TypeScript, Python, Go, and Ruby (debug prints, security patterns, style issues), plus two cross-language rules (`no-todo-comment`, `no-fixme-comment`, `hardcoded-secret`). The `SourceRegistry` / `RuleSource` trait system provides namespace-keyed data (path, env, git, language-specific) used to evaluate `requires` predicates on rules.
