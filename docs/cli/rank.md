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
| `duplicates` | Detect duplicate/similar code (`--scope functions\|blocks`, `--mode exact\|similar\|clusters`) |
| `duplicate-types` | Detect similar type definitions |
| `uniqueness` | Fraction of functions with no structural near-twin per module |
| `fragments` | Find repeated AST fragments |

### Module structure
| Subcommand | Description |
|------------|-------------|
| `size` | Hierarchical LOC breakdown (ncdu-style) |
| `density` | Compression ratio + token uniqueness per module |
| `module-health` | Score each module across test ratio, uniqueness, and density (worst first) |
| `imports` | Rank modules by import fan-in (requires facts index) |
| `depth-map` | Per-module dependency depth + ripple risk |
| `surface` | Per-module public symbol count, public ratio, and constraint score |
| `layering` | Per-module import layering compliance |

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

# Find code duplicates
normalize rank duplicates                                # exact function duplicates
normalize rank duplicates --mode similar                 # similar functions (MinHash)
normalize rank duplicates --scope blocks                 # exact block duplicates
normalize rank duplicates --scope blocks --mode similar  # similar blocks (MinHash)

# Module structure
normalize rank imports            # most-imported modules (requires index)
normalize rank surface            # public API surface per module
normalize rank depth-map          # dependency depth + ripple risk
normalize rank layering           # import layering compliance

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

**duplicates:**
- `--scope functions|blocks` - Detection scope (default: functions)
- `--mode exact|similar|clusters` - Detection mode (default: exact)
- `--elide-identifiers` - Ignore identifier names when comparing
- `--elide-literals` - Ignore literal values when comparing
- `--show-source` - Show source code for matches
- `--min-lines <N>` - Minimum lines to consider
- `--include-trait-impls` - Include same-name groups (likely trait impls)
- `--similarity <F>` - MinHash similarity threshold (similar mode only)
- `--skeleton` - Match on control-flow structure (similar mode only)
- `--repos-dir <DIR>` - Scan across repos under DIR (functions scope only)
- `--skip-functions` - Skip function nodes (blocks scope only)

**hotspots / coupling / ownership:**
- `--repos-dir <DIR>` - Run across all repos under DIR

**hotspots:**
- `--recency` - Weight recent changes higher (exponential decay)

**coupling:**
- `--min-commits <N>` - Minimum shared commits for edges
