# normalize tools lint

Run linters, formatters, and type checkers.

## Usage

```bash
normalize tools lint [PATH]       # Run linters (default)
normalize tools lint run [PATH]   # Explicit run
normalize tools lint list         # List available linters
```

## Options

| Option | Description |
|--------|-------------|
| `--fix` | Auto-fix issues where supported |
| `--json` | JSON output |
| `--only <TOOLS>` | Run only specific tools |
| `--exclude <TOOLS>` | Skip specific tools |

## Examples

```bash
# Lint current directory
normalize tools lint

# Lint specific path
normalize tools lint src/

# With auto-fix
normalize tools lint --fix

# List available tools
normalize tools lint list
```

## Detected Tools

Normalize auto-detects and runs appropriate tools:

| Language | Linters |
|----------|---------|
| Rust | `cargo clippy`, `cargo fmt --check` |
| Python | `ruff`, `mypy`, `pyright` |
| JavaScript/TypeScript | `eslint`, `oxlint`, `tsc` |
| Go | `go vet`, `staticcheck` |

## See Also

- [normalize tools test](test.md) - Run test runners
