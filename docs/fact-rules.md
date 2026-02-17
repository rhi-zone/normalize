# Writing Fact Rules

This guide covers writing fact rules for `normalize`. Fact rules use [Datalog](https://en.wikipedia.org/wiki/Datalog) to query relationships extracted from code — symbols, imports, calls, visibility, and more. They complement [syntax rules](syntax-rules.md), which match AST patterns within individual files.

## Quick Start

Create a `.dl` file in `.normalize/rules/`:

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

Run it:

```bash
normalize facts check                          # Run all .dl rules (auto-discovers)
normalize facts check my-rules.dl              # Run a specific file
normalize rules run --type fact                # Via unified rules command
normalize rules list --type fact               # List fact rules
```

## Rule File Format

Fact rules are `.dl` files with TOML frontmatter in comment blocks:

```datalog
# ---
# id = "my-rule"
# severity = "warning"
# message = "Description shown when rule matches"
# enabled = true
# allow = ["**/tests/**", "**/vendor/**"]
# ---

# Rule logic here...
```

### Frontmatter Fields

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `id` | Yes | - | Unique identifier (e.g., `"circular-deps"`, `"god-file"`) |
| `severity` | No | `"warning"` | `"error"`, `"warning"`, or `"info"` |
| `message` | No | `""` | Description shown when rule matches |
| `enabled` | No | `true` | Set to `false` to disable a builtin rule |
| `allow` | No | `[]` | Glob patterns for files to exclude from findings |

## Available Relations

Facts are extracted from the code index and populated as base relations. All string columns use the `String` type.

### `symbol(file, name, kind, line)`

Every function, class, type, or other definition.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path (relative to root) |
| `name` | String | Symbol name |
| `kind` | String | `"function"`, `"method"`, `"class"`, `"type"`, `"interface"`, etc. |
| `line` | u32 | Line number of the definition |

### `import(from_file, to_module, name)`

Import statements.

| Column | Type | Description |
|--------|------|-------------|
| `from_file` | String | File containing the import |
| `to_module` | String | Module/file being imported |
| `name` | String | Imported name (`"*"` for wildcard imports) |

### `call(caller_file, caller_name, callee_name, line)`

Function calls.

| Column | Type | Description |
|--------|------|-------------|
| `caller_file` | String | File where the call occurs |
| `caller_name` | String | Name of the calling function |
| `callee_name` | String | Name of the called function |
| `line` | u32 | Line number of the call |

### `visibility(file, name, visibility)`

Visibility/access modifiers.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `name` | String | Symbol name |
| `visibility` | String | `"public"`, `"private"`, `"protected"` |

### `attribute(file, name, attribute)`

Decorators, annotations, and attributes.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `name` | String | Symbol the attribute is attached to |
| `attribute` | String | Attribute text (e.g., `"#[test]"`, `"@Override"`) |

### `parent(file, child_name, parent_name)`

Nesting relationships between symbols.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `child_name` | String | Nested symbol name |
| `parent_name` | String | Containing symbol name |

### `qualifier(caller_file, caller_name, callee_name, qualifier)`

Qualified calls (method calls on a type/object).

| Column | Type | Description |
|--------|------|-------------|
| `caller_file` | String | File where the call occurs |
| `caller_name` | String | Name of the calling function |
| `callee_name` | String | Name of the called method |
| `qualifier` | String | Object/type being called on |

### `symbol_range(file, name, start_line, end_line)`

Start and end lines for symbols.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `name` | String | Symbol name |
| `start_line` | u32 | First line of the symbol |
| `end_line` | u32 | Last line of the symbol |

### `implements(file, name, interface)`

Trait/interface implementations.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `name` | String | Implementing type |
| `interface` | String | Interface/trait being implemented |

### `is_impl(file, name)`

Impl blocks (Rust-specific).

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `name` | String | Impl block name |

### `type_method(file, type_name, method_name)`

Methods belonging to a type.

| Column | Type | Description |
|--------|------|-------------|
| `file` | String | File path |
| `type_name` | String | Type that owns the method |
| `method_name` | String | Method name |

## Datalog Syntax

### Relations and Rules

Declare intermediate relations with `relation`, then define rules with `<--`:

```datalog
relation func(String, String);
func(file, name) <-- symbol(file, name, kind, _), if kind == "function";
```

- `_` is a wildcard (matches anything, ignored)
- Multiple clauses in a rule body are joined with `,` (AND logic)

### Guards (`if`)

Filter with `if` conditions:

```datalog
symbol(file, name, kind, _), if kind == "function" || kind == "method";
```

### `let` Bindings

Compute values inline:

```datalog
func_length(file, name, len) <--
    symbol_range(file, name, start, end),
    let len = end - start;
```

### Aggregation

Count, sum, or aggregate with `agg`:

```datalog
relation file_func_count(String, i32);
file_func_count(file, c) <-- func(file, _), agg c = count() in func(file, _);
```

### Negation

Check that a tuple does *not* exist with `!`:

```datalog
warning("orphan-file", file) <-- has_symbols(file), !is_imported(file);
```

### Transitive Closure

Recursion is supported — define a relation in terms of itself:

```datalog
reaches(from, to) <-- import(from, to, _);
reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
```

## Output Relations

To emit diagnostics, insert into the `warning` or `error` output relations:

```datalog
warning("rule-id", entity) <-- /* rule body */;
error("rule-id", entity)   <-- /* rule body */;
```

- First argument: the rule `id` from frontmatter (used for filtering and suppression)
- Second argument: the entity being flagged (file path, symbol name, etc.)

## Inline Suppression

Suppress a fact rule finding on a specific line:

```rust
use some_crate::thing; // normalize-facts-allow: circular-deps
```

The comment `normalize-facts-allow: rule-id` can appear on the same line or the line before. An optional explanation after ` - ` is encouraged:

```python
import foo  # normalize-facts-allow: unused-import - re-exported intentionally
```

## Builtin Fact Rules

normalize ships with 17 builtin fact rules. Rules marked **enabled** run by default; disabled rules can be enabled in config.

### Enabled by Default

| ID | Severity | Description |
|----|----------|-------------|
| `circular-deps` | warning | Circular dependency detected between modules |
| `god-file` | warning | File defines too many functions (>50) |
| `self-import` | warning | File imports itself |

### Disabled by Default

| ID | Severity | Description |
|----|----------|-------------|
| `barrel-file` | warning | File only re-exports from other modules (barrel/index file) |
| `bidirectional-deps` | warning | Two files import each other (bidirectional coupling) |
| `dead-api` | warning | Public function never called from another file |
| `deep-nesting` | warning | Symbol is nested more than 3 levels deep |
| `duplicate-symbol` | warning | Same symbol name defined in multiple files |
| `fan-out` | warning | Function calls too many distinct functions (>50) |
| `god-class` | warning | Type defines too many methods (>20) |
| `hub-file` | warning | Module is imported by many files (>30) |
| `layering-violation` | warning | Test code imports from another test file |
| `long-function` | warning | Function body exceeds 100 lines |
| `missing-export` | warning | Public function is defined but file is never imported |
| `missing-impl` | warning | Class implements interface but is missing required methods |
| `orphan-file` | warning | File is never imported by any other file |
| `unused-import` | warning | Imported name is never referenced in the importing file |

### Overriding Builtins

Enable a disabled builtin or change severity in `.normalize/config.toml`:

```toml
[facts.rules."god-class"]
enabled = true
severity = "error"

[facts.rules."god-file"]
allow = ["**/generated/**"]
```

## Examples

### Detect Circular Dependencies (Transitive)

```datalog
# ---
# id = "circular-deps"
# message = "Circular dependency detected between modules"
# ---

relation reaches(String, String);
relation cycle(String, String);

reaches(from, to) <-- import(from, to, _);
reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
cycle(a, b) <-- reaches(a, b), reaches(b, a), if a < b;

warning("circular-deps", a) <-- cycle(a, _);
```

This uses recursive rules to compute transitive import reachability, then finds cycles.

### Find Unused Public APIs (Negation)

```datalog
# ---
# id = "dead-api"
# message = "Public function never called from another file"
# enabled = false
# allow = ["**/tests/**", "**/main.rs", "**/main.py"]
# ---

relation public_func(String, String);
public_func(file, name) <--
    symbol(file, name, kind, _),
    visibility(file, name, vis),
    if kind == "function" || kind == "method",
    if vis == "public";

relation external_call(String);
external_call(name) <--
    call(caller_file, _, name, _),
    public_func(def_file, name),
    if caller_file != def_file;

warning("dead-api", name) <-- public_func(_, name), !external_call(name);
```

This joins symbols with visibility and call data, using negation to find functions that are public but never called from another file.

### Count Methods per Type (Aggregation)

```datalog
# ---
# id = "god-class"
# message = "Type defines too many methods (>20)"
# enabled = false
# ---

relation method_of(String, String, String);
method_of(file, method, cls) <--
    parent(file, method, cls),
    symbol(file, method, kind, _),
    if kind == "method" || kind == "function";

relation type_method_count(String, String, i32);
type_method_count(file, cls, c) <--
    method_of(file, _, cls),
    agg c = count() in method_of(file, _, cls);

warning("god-class", cls) <-- type_method_count(_, cls, c), if c > 20;
```

## Two Execution Paths

### Interpreted (`.dl` files)

The default. Rules are `.dl` text files run through the built-in Datalog interpreter:

```bash
normalize facts check              # Auto-discover .dl files
normalize facts check rules.dl     # Run specific file
normalize rules run --type fact    # Via unified command
```

Advantages: no compilation, easy to write and share, supports all Datalog features (recursion, aggregation, negation, stratification).

### Compiled (dylib rule packs)

For advanced use, rules can be compiled as Rust dylibs using the [Ascent](https://github.com/s-arash/ascent) macro:

```bash
normalize facts rules              # Run default rule pack
normalize facts rules --pack my_rules.so  # Run custom pack
normalize facts rules --list       # List compiled rules
```

Compiled rules get full Rust expressiveness but require building a crate against `normalize-facts-rules-api`.

## Loading Order

Fact rules are loaded in this order (later rules override earlier ones by `id`):

1. **Embedded builtins** (17 `.dl` rules compiled into the binary)
2. **Global rules** (`~/.config/normalize/rules/*.dl`)
3. **Project rules** (`.normalize/rules/*.dl`)

## See Also

- [Writing Syntax Rules](syntax-rules.md) — tree-sitter query-based rules for AST patterns
- [CLI: facts](cli/facts.md) — `normalize facts` command reference
- [CLI: rules](cli/rules.md) — `normalize rules` unified command reference
