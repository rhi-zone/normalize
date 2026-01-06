# moss edit

Structural code modification using tree-sitter for precise edits.

## Target Syntax

Same as `moss view`:
- `path/to/file` - Edit file
- `file/Symbol` - Edit symbol
- `file/Parent/Child` - Nested symbol
- `@alias` - Edit alias target

## Operations

| Operation | Description |
|-----------|-------------|
| `delete` | Delete target symbol |
| `replace <content>` | Replace target with new content |
| `swap <other>` | Swap target with another symbol |
| `insert <content> --at <pos>` | Insert content relative to target |
| `move <dest> --at <pos>` | Move target to new location |
| `copy <dest> --at <pos>` | Copy target to new location |

Position (`--at`): `before`, `after`, `prepend`, `append`

## Examples

```bash
# Delete a function
moss edit src/old.rs/deprecated_fn delete

# Replace a function
moss edit src/main.rs/parse_config replace "fn parse_config() { todo!() }"

# Swap two functions
moss edit src/lib.rs/foo swap bar

# Insert before a symbol
moss edit src/lib.rs/Config insert "/// Documentation" --at before

# Move function into a class
moss edit src/api.rs/helper move MyClass --at append

# Copy function after another
moss edit src/lib.rs/original copy target --at after
```

## Glob Patterns

Edit multiple symbols matching a pattern:

```bash
# Delete all test_* functions
moss edit "file.py/test_*" delete --multiple

# Replace all foo_* with placeholder
moss edit "file.py/foo*" replace "pass" --multiple

# Insert comment before all matching symbols
moss edit "file.py/deprecated_*" insert "# DEPRECATED" --at before --multiple

# Move all matching symbols into a container
moss edit "file.py/helper_*" move HelperClass --at append --multiple

# Copy all matching symbols after a target
moss edit "file.py/util_*" copy utilities --at after --multiple
```

The `--multiple` flag is required when a pattern matches more than one symbol (safety measure).

### Supported glob characters

- `*` - Match any characters
- `**` - Match across path segments
- `?` - Match single character
- `[...]` - Character class

### Swap not supported with globs

The `swap` operation is not supported with glob patterns because the pairing semantics are ambiguous. If a pattern matches N symbols, what should each swap with? There's no clear answer.

```bash
# This will error:
moss edit "file.py/foo*" swap "file.py/bar*" --multiple
# Error: 'swap' is not supported with glob patterns (ambiguous pairing)
```

For bulk swapping, use multiple individual swap commands or a script.

## Options

### Core
- `--dry-run` - Show what would change without modifying
- `--multiple` - Allow glob patterns matching multiple symbols
- `-y, --yes` - Confirm destructive operations without prompting
- `-m, --message <TEXT>` - Message describing the edit (for shadow git history)
- `-i, --case-insensitive` - Case-insensitive symbol matching

### Undo/Redo (Shadow Git)
- `--undo [<N>]` - Undo the last N edits (default: 1)
- `--redo` - Redo the last undone edit
- `--goto <REF>` - Jump to a specific shadow commit
- `--file <PATH>` - Undo changes only for specific file(s)
- `--cross-checkpoint` - Allow undo across git commit boundaries
- `--force` - Force undo even if files were modified externally

### Batch
- `--batch <FILE>` - Apply batch edits from JSON file (or `-` for stdin)

### Output
- `--json` - Output results as JSON
- `--jq <EXPR>` - Filter JSON with jq expression
- `--pretty` - Human-friendly output
- `--compact` - Compact output without colors
- `-r, --root <PATH>` - Root directory
- `--exclude <PATTERNS>` - Exclude files matching patterns
- `--only <PATTERNS>` - Only include files matching patterns

## Structural vs Text Edits

`moss edit` uses tree-sitter for structural awareness:
- Understands symbol boundaries
- Preserves formatting
- Handles nested structures

For simple text replacements, use standard tools (sed, Edit tool).
