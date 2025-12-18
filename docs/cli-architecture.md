# CLI Architecture

This document describes the internals of the Moss CLI for maintainers.

## Module Structure

**Location**: `src/moss/cli.py`

```
cli.py
├── Output helpers
│   ├── get_version()
│   └── output_result()
├── Project commands
│   ├── cmd_init()
│   ├── cmd_run()
│   ├── cmd_status()
│   ├── cmd_config()
│   └── cmd_distros()
├── Introspection commands
│   ├── cmd_skeleton()
│   ├── cmd_anchors()
│   ├── cmd_query()
│   ├── cmd_cfg()
│   ├── cmd_deps()
│   ├── cmd_context()
│   └── cmd_mcp_server()
├── Helpers
│   └── _symbol_to_dict()
└── Entry points
    ├── create_parser()
    └── main()
```

## Command Pattern

All commands follow this pattern:

```python
def cmd_<name>(args: Namespace) -> int:
    """Docstring describing the command."""
    # 1. Parse arguments from args
    path = Path(args.path).resolve()

    # 2. Validate inputs
    if not path.exists():
        print(f"Error: ...", file=sys.stderr)
        return 1

    # 3. Do work (import dependencies lazily)
    from moss.<module> import <function>
    result = <function>(...)

    # 4. Output (JSON or human-readable)
    if getattr(args, "json", False):
        output_result(result, args)
    else:
        print(formatted_result)

    return 0
```

**Key conventions**:
- Return 0 for success, non-zero for errors
- Errors go to stderr, output to stdout
- Lazy imports to keep CLI startup fast
- All commands support `--json` flag for machine-readable output

## Parser Structure

`create_parser()` builds the argument parser:

```python
parser = argparse.ArgumentParser(prog="moss")
parser.add_argument("--json", "-j", ...)  # Global flag
subparsers = parser.add_subparsers(dest="command")

# Each command
xyz_parser = subparsers.add_parser("xyz", help="...")
xyz_parser.add_argument(...)
xyz_parser.set_defaults(func=cmd_xyz)
```

**Adding a new command**:
1. Write `cmd_<name>(args: Namespace) -> int` function
2. Add parser in `create_parser()` with `subparsers.add_parser()`
3. Set `func=cmd_<name>` as default

## Introspection Commands

### Data Flow

```
User Input
    │
    ▼
┌─────────────┐
│ Path Check  │ → Error if not exists
└─────────────┘
    │
    ▼
┌─────────────┐
│ File/Dir?   │
└─────────────┘
   │        │
   ▼        ▼
Single    Glob Pattern
 File     (default: **/*.py)
   │        │
   ▼        ▼
┌──────────────────┐
│ Process Each File│
│ (try/except for  │
│  syntax errors)  │
└──────────────────┘
    │
    ▼
┌─────────────┐
│ JSON or     │
│ Human Output│
└─────────────┘
```

### Symbol Conversion

`_symbol_to_dict()` converts internal `Symbol` objects to JSON-serializable dicts:

```python
{
    "name": "MyClass",
    "kind": "class",
    "line": 42,
    "signature": "class MyClass(Base):",  # optional
    "docstring": "...",                    # optional
    "children": [...]                      # optional, recursive
}
```

### Output Format

**Human-readable**: Context-appropriate formatting
- skeleton: Indented tree structure
- anchors: `file:line type name (in context)`
- query: Full details with signatures
- deps: Grouped imports/exports
- context: Summary + sections

**JSON** (`--json`): Structured data
- Single file: Object
- Directory: Array of objects
- Consistent schema per command

## Dependencies

Each command lazily imports only what it needs:

| Command | Imports |
|---------|---------|
| skeleton | `moss.skeleton` |
| anchors | `moss.skeleton`, `re` |
| query | `moss.skeleton`, `re` |
| cfg | `moss.cfg` |
| deps | `moss.dependencies` |
| context | `moss.skeleton`, `moss.dependencies` |
| mcp-server | `moss.mcp_server` |

## Error Handling

- **Path errors**: Check existence early, return 1
- **Syntax errors**: Catch per-file, continue processing others
- **Import errors** (MCP): Catch and provide install instructions
- **Keyboard interrupt**: Catch in mcp-server, return 0

## Testing

Tests in `tests/test_cli.py` cover:
- Each command with valid inputs
- JSON output format
- Error cases (missing files, syntax errors)
- Filter combinations

## MCP Server Integration

`cmd_mcp_server()` delegates to `moss.mcp_server.main()` which:
1. Creates MCP server with tool definitions
2. Registers handlers for each tool
3. Runs stdio transport loop

The MCP tools mirror CLI commands but return structured data directly.
