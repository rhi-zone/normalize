# normalize rank

Rank code by metrics — complexity, size, coupling, duplicates, and more.

Every `rank` subcommand produces an ordered list of items by some metric. These
commands were previously under `normalize analyze`.

## Subcommands

### Code quality
| Subcommand | Description |
|------------|-------------|
| `complexity` | Cyclomatic complexity per function |
| `call-complexity` | Effective (reachable) cyclomatic complexity via call-graph BFS |
| `ceremony` | Fraction of callables that are trait/interface boilerplate |
| `uniqueness` | Fraction of functions with no structural near-twin per module |

`duplicates`, `duplicate-types`, and `fragments` moved to the top-level
[`normalize similarity`](../cli-design.md) verb (`similarity` incl. `--mode clusters`,
`similarity duplicate-types`, `similarity fragments`; owned by `normalize-code-similarity`,
index-free). Old `rank` paths remain as hidden aliases for one release.

### Module structure
| Subcommand | Description |
|------------|-------------|
| `size` | Hierarchical LOC breakdown (ncdu-style) |
| `density` | Compression ratio + token uniqueness per module |
| `module-health` | Score each module across test ratio, uniqueness, and density (worst first) |
| `imports` | Rank modules by import fan-in (requires facts index) |
| `surface` | Per-module public symbol count, public ratio, and constraint score |

`depth-map` and `layering` moved to the top-level [`normalize architecture`](../cli-design.md)
verb (`architecture depth-map` / `architecture layering`; old `rank` paths remain as hidden
aliases for one release).

### Repository
| Subcommand | Description |
|------------|-------------|
| `files` | Longest files in codebase |

### Git history
| Subcommand | Description |
|------------|-------------|
| `hotspots` | Churn × complexity hotspots |
| `coupling` | Temporal coupling: file pairs that change together |
| `ownership` | Per-file ownership concentration from git blame |
| `contributors` | Analyze contributors across repos |

### Testing
| Subcommand | Description |
|------------|-------------|
| `test-ratio` | Test/impl line ratio per module |
| `budget` | Line budget breakdown by purpose (logic, tests, docs, config) |

## Examples

```bash
# Find complex functions
normalize rank complexity --threshold 15

# Find hotspot files
normalize rank hotspots

# Temporal coupling analysis
normalize rank coupling

# Information density
normalize rank density

# Find code duplicates moved to normalize similarity:
normalize similarity                                # exact function duplicates
normalize similarity --mode similar                 # similar functions (MinHash)
normalize similarity --scope blocks                 # exact block duplicates
normalize similarity --mode clusters                # near-duplicate clusters
normalize similarity duplicate-types                # duplicate type definitions
normalize similarity fragments                      # repeated AST fragments

# Module structure
normalize rank imports            # most-imported modules (requires index)
normalize rank surface            # public API surface per module

# Dependency depth + layering moved to normalize architecture:
normalize architecture depth-map  # dependency depth + ripple risk
normalize architecture layering   # import layering compliance

# Test coverage
normalize rank test-ratio         # test/impl ratio per module
normalize rank budget             # line budget breakdown

# Show worst modules overall
normalize rank module-health
```

## Options

### Global
- `-r, --root <PATH>` - Root directory
- `-l, --limit <N>` - Maximum results to show (0=no limit)
- `--diff <REF>` - Show delta vs git ref
- `--json` - Output as JSON
- `--jsonl` - Output one JSON object per line
- `--jq <EXPR>` - Filter JSON with jq
- `--pretty` - Human-friendly output
- `--compact` - Compact output without colors
- `--exclude <PATTERN>` - Exclude paths
- `--only <PATTERN>` - Include only paths
- `--input-schema` - Print JSON Schema of input parameters
- `--output-schema` - Print JSON Schema of return type
- `--params-json <JSON>` - Provide all parameters as a JSON object

### Subcommand-specific

**complexity:**
- `-t, --threshold <N>` - Only show functions above threshold

(`duplicates` flags moved with the command to `normalize similarity` — run
`normalize similarity --help`.)

**hotspots / coupling / ownership:**
- `--repos-dir <DIR>` - Run across all repos under DIR

**hotspots:**
- `--recency` - Weight recent changes higher (exponential decay)

**coupling:**
- `--min-commits <N>` - Minimum shared commits for edges
