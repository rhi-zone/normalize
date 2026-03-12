# normalize analyze

Analyze codebase quality: health, complexity, security, duplicates, docs.

## Subcommands

### Health & Scoring
| Subcommand | Description |
|------------|-------------|
| `health` | File counts, complexity stats, large file warnings (default when no subcommand) |
| `module-health` | Score each module across test ratio, uniqueness, and density |
| `cross-repo-health` | Rank repos by tech debt (churn + complexity + coupling) |
| `summary` | Auto-generated single-page codebase overview |
| `trend` | Track health metrics over git history at regular intervals |
| `all` | Run all analysis passes with overall grade |

### Complexity
| Subcommand | Description |
|------------|-------------|
| `complexity` | Cyclomatic complexity per function |
| `call-complexity` | Effective (reachable) cyclomatic complexity via call-graph BFS |
| `length` | Function length analysis |

### Duplicates & Similarity
| Subcommand | Description |
|------------|-------------|
| `duplicates` | Detect duplicate/similar code (`--scope functions\|blocks`, `--similar`) |
| `duplicate-types` | Detect similar type definitions |
| `fragments` | Find repeated AST fragments (`--scope`, `--skeleton`, `--similarity`, `--min-members`) |

### Coverage & Testing
| Subcommand | Description |
|------------|-------------|
| `test-ratio` | Test/impl line ratio per module |
| `test-gaps` | Untested public functions ranked by risk |
| `budget` | Line budget breakdown by purpose (logic, tests, docs, config) |

### Information Density
| Subcommand | Description |
|------------|-------------|
| `density` | Compression ratio + token uniqueness per module |
| `uniqueness` | Fraction of functions with no structural near-twin per module |
| `ceremony` | Ceremony ratio: fraction of callables that are trait/interface boilerplate |

### Churn & Coupling
| Subcommand | Description |
|------------|-------------|
| `coupling` | Temporal coupling: file pairs that change together |
| `coupling-clusters` | Change-clusters: connected components of coupled files |
| `hotspots` | Churn × complexity hotspots |
| `ownership` | Per-file ownership concentration from git blame |

### Dependencies & Structure
| Subcommand | Description |
|------------|-------------|
| `imports` | Rank modules by import fan-in (requires facts index) |
| `depth-map` | Per-module dependency depth + ripple risk |
| `surface` | Per-module public symbol count, public ratio, and constraint score |
| `layering` | Per-module import layering compliance |
| `architecture` | Codebase architecture: coupling, cycles, dependencies |
| `graph` | Graph-theoretic properties of the dependency graph (`--on modules\|symbols\|types`) |
| `call-graph` | Show callers and/or callees of a symbol (`--callers`, `--callees`) |
| `trace` | Trace value provenance for a symbol |
| `impact` | What-if impact analysis: reverse-dependency closure + blast radius |

### Documentation
| Subcommand | Description |
|------------|-------------|
| `docs` | Documentation coverage |

### Cross-cutting
| Subcommand | Description |
|------------|-------------|
| `security` | Security vulnerability patterns |
| `files` | Longest files in codebase |
| `size` | Hierarchical LOC breakdown (ncdu-style) |
| `skeleton-diff` | Structural changes between a base ref and HEAD |
| `provenance` | Git blame → session mapping + code relations |
| `activity` | Cross-repo activity over time |
| `contributors` | Analyze contributors across repos |
| `repo-coupling` | Analyze cross-repo coupling |

## Examples

```bash
# Quick health check
normalize analyze

# Test coverage
normalize analyze test-ratio                  # test/impl ratio per module
normalize analyze test-gaps                   # untested public functions
normalize analyze budget                      # line budget breakdown

# Churn analysis
normalize analyze coupling                    # temporal coupling pairs
normalize analyze coupling-clusters           # change-clusters
normalize analyze hotspots                    # churn × complexity hotspots

# Architecture analysis
normalize analyze architecture

# Find complex functions
normalize analyze complexity --threshold 15

# Security scan
normalize analyze security

# Find code duplicates
normalize analyze duplicates                                # exact function duplicates
normalize analyze duplicates --similar                      # similar functions (MinHash)
normalize analyze duplicates --scope blocks                 # exact block duplicates
normalize analyze duplicates --scope blocks --similar       # similar blocks (MinHash)

# Trace a symbol's data flow
normalize analyze trace parse_config

# Call graph
normalize analyze call-graph handle_request --callers
normalize analyze call-graph handle_request --callees

# Dependency graph analysis
normalize analyze graph                              # Module-level graph
normalize analyze graph --on symbols                 # Symbol-level graph
normalize analyze graph --on types                   # Type dependency graph

# Impact analysis
normalize analyze impact src/main.rs
```

## Options

### Global
- `-r, --root <PATH>` - Root directory
- `--json` - Output as JSON
- `--jsonl` - Output one JSON object per line
- `--jq <EXPR>` - Filter JSON with jq
- `--pretty` - Human-friendly output
- `--compact` - Compact output without colors
- `--exclude <PATTERN>` - Exclude paths
- `--only <PATTERN>` - Include only paths
- `--diff [<BASE>]` - Analyze only files changed since base ref (default: origin's default branch)
- `--input-schema` - Print JSON Schema of input parameters
- `--output-schema` - Print JSON Schema of return type
- `--params-json <JSON>` - Provide all parameters as a JSON object

### Subcommand-specific

**complexity:**
- `-t, --threshold <N>` - Only show functions above threshold
- `--kind <TYPE>` - Filter by: function, method

**test-gaps:**
- `--all` - Show all functions including tested
- `--min-risk <N>` - Risk threshold

**coupling / coupling-clusters:**
- `--min-commits <N>` - Minimum shared commits for edges

**hotspots:**
- `--recency` - Weight recent changes higher (exponential decay)

**files:**
- `--allow <PATTERN>` - Add pattern to allow file
- `--reason <TEXT>` - Reason for allowing (with --allow)
- `-n, --limit <N>` - Number of results to show

**duplicates:**
- `--scope functions|blocks` - Detection scope (default: functions)
- `--similar` - Use fuzzy MinHash matching instead of exact hash
- `--elide-identifiers` - Ignore identifier names when comparing
- `--elide-literals` - Ignore literal values when comparing
- `--show-source` - Show source code for matches
- `--min-lines <N>` - Minimum lines to consider (default: 1 exact-functions, 5 exact-blocks, 15 similar-blocks, 10 similar-functions/clusters; configurable via `[analyze.duplicates] min_lines`)
- `--include-trait-impls` - Include same-name groups (likely trait impls)
- `--similarity <F>` - MinHash similarity threshold (similar mode only)
- `--skeleton` - Match on control-flow structure (similar mode only)
- `--repos <DIR>` - Scan across sibling repos (functions scope only)
- `--skip-functions` - Skip function nodes (blocks scope only)

**trace:**
- `--target <FILE>` - Target file to search in
- `--max-depth <N>` - Maximum trace depth (default: 10)
- `--recursive` - Trace into called functions

## Allow Files

Patterns can be excluded via `.normalize/` allow files:

| File | Purpose |
|------|---------|
| `.normalize/large-files-allow` | Exclude from `analyze files` |
| `.normalize/hotspots-allow` | Exclude from `analyze hotspots` |
| `.normalize/duplicate-functions-allow` | Exclude from `analyze duplicates --scope functions` |
| `.normalize/duplicate-types-allow` | Exclude type pairs |
| `.normalize/test-gaps-allow` | Exclude from `analyze test-gaps` |

Add via CLI:
```bash
normalize analyze files --allow "**/generated/*.rs" --reason "generated code"
```

## Config

In `.normalize/config.toml`:

```toml
[analyze]
threshold = 10           # Default complexity threshold
compact = false          # Compact overview output
health = true            # Run health by default
complexity = true        # Run complexity by default
security = true          # Run security by default
duplicate_functions = false
exclude_interface_impls = true  # Exclude trait impls from doc coverage
hotspots_exclude = ["*.lock", "CHANGELOG.md"]

[analyze.weights]
health = 1.0
complexity = 0.5
security = 2.0
duplicate_functions = 0.3
```
