# normalize grammars

Manage tree-sitter grammars for parsing.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List available grammars |
| `install` | Install grammars from GitHub release |
| `info <LANG>` | Show grammar info |
| `check` | Verify grammars are working |

## Examples

```bash
# List all grammars
normalize grammars list

# Install grammars
normalize grammars install                    # install latest
normalize grammars install --version v0.1.0   # specific version
normalize grammars install --force            # reinstall
normalize grammars install --dry-run          # preview without downloading

# Grammar info
normalize grammars info rust
normalize grammars info typescript

# Verify
normalize grammars check
```

## Supported Languages

Normalize includes grammars for 90+ languages via arborium.
See `normalize grammars list` for the full list.
