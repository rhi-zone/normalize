# Builtin Syntax Rules

Design for built-in syntax linting rules shipped with moss.

## Rule Loading Order

Rules are loaded in this order (later overrides earlier by `id`):

1. **Embedded builtins** - compiled into the moss binary
2. **User global** - `~/.config/moss/rules/*.scm`
3. **Project** - `.moss/rules/*.scm`

To disable a builtin, create a rule with same `id` and `enabled = false`:

```scm
# ---
# id = "no-todo-comment"
# enabled = false
# ---
```

## Proposed Builtin Rules

### Rust-specific

#### `rust/todo-macro` (warning)
Detects `todo!()` and `unimplemented!()` macros in non-test code.

```scheme
((macro_invocation
  macro: (identifier) @_name
  (#any-of? @_name "todo" "unimplemented")) @match)
```

Allow: `**/tests/**`, `**/*_test.rs`, `**/test_*.rs`

#### `rust/println-debug` (info)
Detects `println!`, `print!`, `dbg!` - prefer tracing/log crate.

```scheme
((macro_invocation
  macro: (identifier) @_name
  (#any-of? @_name "println" "print" "dbg" "eprint" "eprintln")) @match)
```

Allow: `**/tests/**`, `**/examples/**`, `**/bin/**`

#### `rust/expect-empty` (warning)
Detects `.expect("")` with empty string - provide context.

```scheme
((call_expression
  function: (field_expression field: (field_identifier) @_method)
  arguments: (arguments (string_literal) @_msg)
  (#eq? @_method "expect")
  (#eq? @_msg "\"\"")) @match)
```

#### `rust/unwrap-in-impl` (info)
Detects `.unwrap()` outside tests - consider `?` or `.expect()`.

```scheme
((call_expression
  function: (field_expression field: (field_identifier) @_method)
  (#eq? @_method "unwrap")) @match)
```

Allow: `**/tests/**`, `**/examples/**`

### JavaScript/TypeScript

#### `js/console-log` (info)
Detects `console.log`, `console.debug` - remove before commit.

```scheme
((call_expression
  function: (member_expression
    object: (identifier) @_obj
    property: (property_identifier) @_prop)
  (#eq? @_obj "console")
  (#any-of? @_prop "log" "debug" "info")) @match)
```

Allow: `**/tests/**`, `**/*.test.*`, `**/*.spec.*`

### Cross-language

#### `no-todo-comment` (info)
Detects TODO/FIXME/XXX/HACK comments.

```scheme
; Rust
((line_comment) @match (#match? @match "TODO|FIXME|XXX|HACK"))

; JavaScript/TypeScript (same pattern works)
((comment) @match (#match? @match "TODO|FIXME|XXX|HACK"))
```

#### `no-fixme-comment` (warning)
Specifically FIXME - higher severity as it indicates known bugs.

```scheme
((line_comment) @match (#match? @match "FIXME"))
```

### Code Quality

#### `rust/unnecessary-let` (info)
Detects `let x = y;` where `x` is an identifier binding to another identifier, and binding is immutable.

```scheme
((let_declaration
  pattern: (identifier) @_alias
  value: (identifier) @_value
  (#not-match? @_alias "^_")) @match)
```

Caveats:
- May flag legitimate renamings for clarity
- Severity `info` - informational only
- Users can allowlist specific patterns

#### `rust/unnecessary-type-alias` (info)
Detects `type X = Y;` where both are simple type identifiers.

```scheme
(type_alias_declaration
  name: (type_identifier) @_alias
  type: (type_identifier) @_target) @match
```

Caveats:
- Legitimate uses: re-exports, shortening long paths
- Severity `info`

#### `ts/unnecessary-const` (info)
Detects `const x = y;` where both are identifiers.

```scheme
((lexical_declaration
  kind: "const"
  (variable_declarator
    name: (identifier) @_alias
    value: (identifier) @_value)) @match)
```

### Security

#### `hardcoded-secret` (error)
Detects potential hardcoded secrets in string assignments.

```scheme
; Rust
((let_declaration
  pattern: (identifier) @_name
  value: (string_literal) @_value
  (#match? @_name "(?i)password|secret|api.?key|token")
  (#not-match? @_value "^\"\"$")) @match)
```

Note: High false positive rate expected. Severity `error` but heavily allowlisted.

## Implementation Notes

### Embedding Rules

Rules are embedded using `include_str!`:

```rust
const BUILTIN_RULES: &[(&str, &str)] = &[
    ("rust/todo-macro", include_str!("rules/rust/todo-macro.scm")),
    ("rust/println-debug", include_str!("rules/rust/println-debug.scm")),
    // ...
];
```

### Language Inference

Rules in `rust/` subdirectory automatically get `languages = ["rust"]`.
Rules in `js/` get `languages = ["javascript", "typescript", "tsx", "jsx"]`.
Root-level rules try all grammars.

### Testing

Each builtin rule needs test cases:
- Positive cases (should match)
- Negative cases (should not match)
- Edge cases (comments, strings, etc.)

## Open Questions

1. Should builtins be opt-in or opt-out?
   - Opt-out (enabled by default) catches more issues
   - Opt-in (disabled by default) is less noisy for new users
   - Proposal: opt-out with easy global disable

2. Naming convention for rule IDs?
   - `rust/rule-name` - language-prefixed
   - `rule-name` - flat namespace
   - Proposal: language-prefixed for clarity

3. Where to store embedded rule files?
   - `crates/moss/src/commands/analyze/rules/` - next to rules.rs
   - `rules/` at crate root
   - Inline as strings in code
   - Proposal: `crates/moss/src/commands/analyze/builtin_rules/`
