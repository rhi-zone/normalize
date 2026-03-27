# normalize trend

Track health metrics over git history. Shows how complexity, function length, information density, and test coverage have changed across commits.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `multi` | Track all metrics (complexity, length, test ratio, density) in one report |
| `complexity` | Average cyclomatic complexity trend over git history |
| `length` | Average function length trend over git history |
| `density` | Information density trend over git history |
| `test-ratio` | Test-to-code ratio trend over git history |

## Examples

```bash
# All metrics together
normalize trend multi

# Specific metric
normalize trend complexity
normalize trend test-ratio

# JSON for visualization
normalize trend multi --json
normalize trend complexity --json | jq '.data[] | [.commit, .value]'
```

## Options

### Global
- `-r, --root <PATH>` - Root directory (defaults to cwd)
- `--json` - Output as JSON
- `--jsonl` - Output one JSON object per line
- `--jq <EXPR>` - Filter JSON with jq
- `--pretty` - Human-friendly output with colors
- `--compact` - Compact output without colors
- `--input-schema` - Print JSON Schema of input parameters
- `--output-schema` - Print JSON Schema of return type
- `--params-json <JSON>` - Provide all parameters as a JSON object

## Notes

- Requires git history; results vary based on number of commits and repo size.
- Previously these commands were under `normalize analyze` (`analyze complexity-trend`, `analyze length-trend`, etc.). They moved to `normalize trend` for better discoverability.
