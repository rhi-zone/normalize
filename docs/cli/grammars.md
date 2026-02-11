# normalize grammars

Manage tree-sitter grammars for parsing.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List available grammars |
| `info <LANG>` | Show grammar info |
| `check` | Verify grammars are working |

## Examples

```bash
# List all grammars
normalize grammars list

# Grammar info
normalize grammars info rust
normalize grammars info typescript

# Verify
normalize grammars check
```

## Supported Languages

Normalize includes grammars for 90+ languages via arborium.
See `normalize grammars list` for the full list.
