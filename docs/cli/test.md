# normalize tools test

Run native test runners for detected languages.

## Usage

```bash
normalize tools test [PATH] [-- ARGS]   # Run tests (default)
normalize tools test run [PATH]         # Explicit run
normalize tools test list               # List available runners
```

## Options

| Option | Description |
|--------|-------------|
| `--runner <NAME>` | Use specific test runner |
| `--json` | JSON output |
| `-r, --root <PATH>` | Root directory |

## Examples

```bash
# Run all tests
normalize tools test

# Run tests in path
normalize tools test src/

# Pass args to test runner
normalize tools test -- --nocapture
normalize tools test -- -v

# List available runners
normalize tools test list
```

## Detected Runners

| Language | Runner |
|----------|--------|
| Rust | `cargo test` |
| Go | `go test` |
| Python | `pytest`, `unittest` |
| JavaScript | `bun test`, `vitest`, `jest` |

## See Also

- [normalize tools lint](lint.md) - Run linters
