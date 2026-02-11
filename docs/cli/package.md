# normalize package

Package management utilities: info, dependencies, outdated checks.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `info <PKG>` | Show package information |
| `list` | List project dependencies |
| `tree` | Show dependency tree |
| `outdated` | Check for outdated dependencies |

## Examples

```bash
# Package info
normalize package info serde
normalize package info react

# List dependencies
normalize package list

# Dependency tree
normalize package tree
normalize package tree --depth 2

# Check outdated
normalize package outdated
```

## Supported Ecosystems

| Ecosystem | Manifest |
|-----------|----------|
| Rust | Cargo.toml |
| Node.js | package.json |
| Python | pyproject.toml, requirements.txt |
| Go | go.mod |

## Options

- `--json` - JSON output
- `-r, --root <PATH>` - Root directory
