# Rules

Normalize has two types of analysis rules that catch issues across your codebase:

| Type | File Extension | Engine | Scope |
|------|---------------|--------|-------|
| **Syntax rules** | `.scm` | Tree-sitter queries | Single-file AST patterns |
| **Fact rules** | `.dl` | Datalog (Ascent) | Cross-file relationships |

## Quick Start

```bash
# Run all rules (syntax + fact)
normalize rules run

# List available rules
normalize rules list

# Add a rule from a URL
normalize rules add https://example.com/rules/no-console-log.scm

# Run only one type
normalize rules run --type syntax
normalize rules run --type fact
```

## Syntax Rules

Syntax rules match AST patterns within individual files using tree-sitter queries. Good for catching code patterns like `unwrap()` calls, debug statements, or hardcoded secrets.

```scheme
# ---
# id = "rust/todo-macro"
# severity = "warning"
# message = "todo!() macro found"
# languages = ["rust"]
# ---

(macro_invocation macro: (identifier) @_name
  (#eq? @_name "todo")) @match
```

Normalize ships with 24 builtin syntax rules for Rust, JavaScript/TypeScript, Python, Go, and Ruby.

**Guide:** [Writing Syntax Rules](syntax-rules.md)

## Fact Rules

Fact rules query relationships extracted from code — symbols, imports, calls, visibility — using Datalog. Good for cross-file analysis like detecting circular dependencies, god files, or unused exports.

```datalog
# ---
# id = "too-many-imports"
# severity = "warning"
# message = "File imports more than 20 modules"
# ---

relation import_count(String, i32);
import_count(file, c) <-- import(file, _, _), agg c = count() in import(file, _, _);
warning("too-many-imports", file) <-- import_count(file, c), if c > 20;
```

Normalize ships with builtin fact rules for common issues (god files, circular deps, etc.).

**Guide:** [Writing Fact Rules](fact-rules.md)

## Rule Storage

Rules are loaded in priority order (later override earlier by `id`):

1. **Embedded builtins** — compiled into the binary
2. **Global rules** — `~/.config/normalize/rules/*.scm` and `*.dl`
3. **Project rules** — `.normalize/rules/*.scm` and `*.dl`

Imported rules are tracked in `.normalize/rules.lock`.

## Configuration

Override rule settings in `.normalize/config.toml`:

```toml
[analyze.rules."rust/unwrap-in-impl"]
severity = "error"          # Change severity
enabled = false             # Disable rule
allow = ["**/tests/**"]     # Skip these paths
```

## Inline Suppression

Suppress findings with comments:

```rust
// normalize-syntax-allow: rust/unwrap-in-impl - validated above
let x = result.unwrap();
```

```python
# normalize-facts-allow: god-file - intentionally large
```

## See Also

- [Writing Syntax Rules](syntax-rules.md) — Full guide for `.scm` rules
- [Writing Fact Rules](fact-rules.md) — Full guide for `.dl` rules
- [CLI: normalize rules](cli/rules.md) — Command reference
