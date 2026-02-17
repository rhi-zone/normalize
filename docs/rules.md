# Writing Syntax Rules

This guide covers writing custom analysis rules for `normalize analyze rules`. Rules use [tree-sitter](https://tree-sitter.github.io/) queries with TOML frontmatter for metadata.

## Quick Start

Create a rule file in `.normalize/rules/`:

```scheme
# ---
# id = "no-todo-comment"
# severity = "info"
# message = "TODO comment found"
# ---

(line_comment) @match
(#match? @match "TODO")
```

Run it:

```bash
normalize analyze rules
normalize analyze rules --rule no-todo-comment  # Run specific rule
normalize analyze rules --fix                   # Apply auto-fixes
normalize analyze rules --list                  # List all rules
```

## Rule File Format

Rules are `.scm` files with TOML frontmatter in comment blocks:

```scheme
# ---
# id = "rust/unwrap-in-impl"
# severity = "warning"
# message = "Avoid .unwrap() in production code"
# languages = ["rust"]
# allow = ["**/tests/**", "**/examples/**"]
# ---

(call_expression
  (field_expression field: (field_identifier) @_method)
  (#eq? @_method "unwrap")) @match
```

### Frontmatter Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `id` | Yes | - | Unique identifier (convention: `language/rule-name` or `rule-name` for cross-language) |
| `severity` | No | `"warning"` | `"error"`, `"warning"`, or `"info"` |
| `message` | No | `""` | Description shown when rule matches |
| `languages` | No | all | Array of languages this rule applies to (e.g., `["rust"]`, `["javascript", "typescript"]`) |
| `allow` | No | `[]` | Glob patterns for files to skip (e.g., `["**/tests/**"]`) |
| `enabled` | No | `true` | Set to `false` to disable a builtin rule |
| `requires` | No | `{}` | Conditional execution (see [Conditionals](#conditionals)) |
| `fix` | No | - | Auto-fix template (see [Auto-fix](#auto-fix)) |

## Tree-sitter Queries

Rules use tree-sitter's S-expression query syntax. The `@match` capture is **required** - it marks the location reported in findings.

### Basic Patterns

Match a specific node type:

```scheme
(function_definition) @match
```

Match with field constraints:

```scheme
(call_expression
  function: (identifier) @_fn
  (#eq? @_fn "eval")) @match
```

Match nested structures:

```scheme
(if_statement
  consequence: (block
    (return_statement))) @match
```

### Predicates

| Predicate | Description | Example |
|-----------|-------------|---------|
| `#eq?` | Exact string match | `(#eq? @name "foo")` |
| `#not-eq?` | Negated exact match | `(#not-eq? @name "test")` |
| `#match?` | Regex match | `(#match? @name "^test_")` |
| `#not-match?` | Negated regex match | `(#not-match? @val "^$")` |
| `#any-of?` | Match any of multiple values | `(#any-of? @name "foo" "bar" "baz")` |

### Capture Naming

- `@match` - **Required.** The node whose location is reported.
- `@_name` - Captures starting with `_` are used in predicates but not reported.
- `@name` - Named captures are available in auto-fix templates via `$name`.

### Authoring Helpers

Use `normalize analyze ast` and `normalize analyze query` to develop rules interactively:

```bash
# Dump the full AST for a file
normalize analyze ast src/main.rs

# Show AST at a specific line
normalize analyze ast src/main.rs --at 42

# Test a query interactively
normalize analyze query src/main.rs '(call_expression function: (identifier) @fn) @match'

# Test with source context
normalize analyze query src/main.rs --show-source '(function_item) @match'
```

## Conditionals

The `requires` field enables rules only when conditions are met:

```toml
requires = { "rust.edition" = ">=2024", "env.CI" = "true" }
```

All conditions must be satisfied (AND logic).

### Operators

| Syntax | Meaning | Example |
|--------|---------|---------|
| `"value"` | Exact match | `"rust.edition" = "2021"` |
| `">=value"` | Greater or equal | `"rust.edition" = ">=2024"` |
| `"<=value"` | Less or equal | `"go.version" = "<=1.21"` |
| `"!value"` | Not equal | `"git.branch" = "!main"` |

### Available Sources

| Namespace | Keys | Description |
|-----------|------|-------------|
| `env` | Any env var | Environment variables (`env.CI`, `env.NODE_ENV`) |
| `path` | `rel`, `abs`, `ext`, `filename` | File path components |
| `git` | `branch`, `staged`, `dirty` | Git repository state |
| `rust` | `edition`, `resolver`, `name`, `version`, `is_test_file` | From nearest `Cargo.toml` |
| `typescript` | `target`, `module`, `strict`, `moduleResolution`, `name`, `version`, `node_version` | From `tsconfig.json` + `package.json` |
| `python` | `requires_python`, `name`, `version` | From `pyproject.toml` |
| `go` | `version`, `module` | From `go.mod` |

### Examples

Only run in CI:

```toml
requires = { "env.CI" = "true" }
```

Rust 2024+ only (chained if-let):

```toml
requires = { "rust.edition" = ">=2024" }
```

Not on main branch:

```toml
requires = { "git.branch" = "!main" }
```

## Auto-fix

The `fix` field defines a replacement template applied to the `@match` capture:

```toml
fix = ""              # Delete the match
fix = "$_arg"         # Replace with a capture's text
fix = "log.debug($1)" # Replace with literal + captures
```

Captures from the query are available as `$capture_name`. The special `$match` refers to the full matched text.

### Example: Replace unwrap with expect

```scheme
# ---
# id = "rust/unwrap-to-expect"
# severity = "info"
# message = "Use .expect() with a message instead of .unwrap()"
# languages = ["rust"]
# fix = ".expect(\"TODO: add error message\")"
# ---

(call_expression
  (field_expression field: (field_identifier) @_method)
  (#eq? @_method "unwrap")) @match
```

### Example: Delete debug statements

```scheme
# ---
# id = "python/remove-breakpoint"
# severity = "warning"
# message = "Remove breakpoint() before committing"
# languages = ["python"]
# fix = ""
# ---

(call
  function: (identifier) @_fn
  (#eq? @_fn "breakpoint")) @match
```

Run fixes:

```bash
normalize analyze rules --fix                       # Fix all
normalize analyze rules --rule rust/unwrap-to-expect --fix  # Fix one rule
```

Fixes are applied in reverse byte-offset order to preserve positions. Multiple fixes to the same file are applied in one pass.

## Inline Suppression

Suppress a rule on a specific line with a comment:

```rust
let x = dangerous_unwrap(); // normalize-syntax-allow: rust/unwrap-in-impl

// normalize-syntax-allow: rust/unwrap-in-impl - justified because of error handling above
let y = safe_unwrap();
```

The comment can be on the same line as the finding or the line immediately before it. An optional explanation after ` - ` is encouraged.

## Configuration

Override rule settings in `.normalize/config.toml`:

```toml
[analyze.rules."rust/println-debug"]
severity = "warning"    # Upgrade severity
enabled = false         # Disable rule

[analyze.rules."rust/unwrap-in-impl"]
allow = ["**/cmd/**"]   # Additional allow patterns
```

## Rule Sharing

Import rules from URLs:

```bash
# Add a rule
normalize rules add https://example.com/rules/security.scm
normalize rules add https://example.com/rules/style.scm --global

# List installed rules
normalize rules list
normalize rules list --sources

# Update from source
normalize rules update

# Remove
normalize rules remove security
```

Imported rules are tracked in `.normalize/rules.lock` (project) or `~/.config/normalize/rules.lock` (global).

## Loading Order

Rules are loaded in this order (later rules override earlier ones by `id`):

1. **Embedded builtins** (compiled into the binary)
2. **Global rules** (`~/.config/normalize/rules/*.scm`)
3. **Project rules** (`.normalize/rules/*.scm`)

## Builtin Rules

normalize ships with 24 builtin rules:

### Rust

| ID | Severity | Description |
|----|----------|-------------|
| `rust/todo-macro` | warning | `todo!()` macro calls |
| `rust/dbg-macro` | warning | `dbg!()` macro calls |
| `rust/println-debug` | info | `println!`/`eprintln!` (allows tests, examples, bin) |
| `rust/unwrap-in-impl` | info | `.unwrap()` calls (allows tests, examples, benches) |
| `rust/expect-empty` | warning | `.expect("")` with empty message |
| `rust/unnecessary-let` | info | `let x = y;` identity bindings |
| `rust/unnecessary-type-alias` | info | `type Foo = Bar;` trivial aliases |
| `rust/chained-if-let` | info | Nested if-let (requires `rust.edition >= "2024"`) |
| `rust/numeric-type-annotation` | info | Redundant numeric type annotations |
| `rust/tuple-return` | info | Functions returning tuples instead of structs |

### JavaScript / TypeScript

| ID | Severity | Description |
|----|----------|-------------|
| `js/console-log` | info | `console.log/debug/info` (allows tests) |
| `js/unnecessary-const` | info | `const x = y;` identity bindings |
| `typescript/tuple-return` | info | Tuple return types |

### Python

| ID | Severity | Description |
|----|----------|-------------|
| `python/print-debug` | info | `print()` calls (allows tests, examples) |
| `python/breakpoint` | warning | `breakpoint()` calls |
| `python/tuple-return` | info | Tuple return types |

### Go

| ID | Severity | Description |
|----|----------|-------------|
| `go/fmt-print` | info | `fmt.Print/Println/Printf` (allows tests, cmd) |
| `go/many-returns` | info | Functions with many return values |

### Ruby

| ID | Severity | Description |
|----|----------|-------------|
| `ruby/binding-pry` | warning | `binding.pry`/`binding.irb` debug calls |

### Cross-language

| ID | Severity | Description |
|----|----------|-------------|
| `hardcoded-secret` | error | Variables named password/secret/token with string values |
| `no-todo-comment` | info | `// TODO` comments |
| `no-fixme-comment` | warning | `// FIXME` comments |

## Tips

- **Start with `normalize analyze ast`** to understand the AST structure of your target language.
- **Use `@_` prefixed captures** for predicate-only nodes to keep findings focused.
- **Cross-language rules** work when the query nodes exist in the grammar. A rule without `languages` is validated per-grammar and silently skipped for incompatible languages.
- **Test rules incrementally** with `normalize analyze query` before adding frontmatter.
- **Prefer `#match?` over `#eq?`** when you need partial matching or case-insensitive patterns.

## See Also

- [Writing Fact Rules](fact-rules.md) — Datalog-based rules for cross-file analysis
- [CLI: rules](cli/rules.md) — Unified `normalize rules` command reference
- [CLI: analyze rules](cli/rules.md) — Command reference
- [Design: syntax linting](design/syntax-linting.md) — Architecture decisions
- [Design: builtin rules](design/builtin-rules.md) — Builtin rule development
