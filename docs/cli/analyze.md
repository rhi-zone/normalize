# normalize analyze

Analyze codebase quality: health, security, docs, call graphs, and more.

Ranked-list commands (complexity, size, coupling, duplicates, etc.) have moved to
[`normalize rank`](rank.md).

## Subcommands

### Health & Scoring
| Subcommand | Description |
|------------|-------------|
| `health` | File counts, complexity stats, large file warnings (default when no subcommand) |
| `cross-repo-health` | Rank repos by tech debt (churn + complexity + coupling) |
| `summary` | Auto-generated single-page codebase overview |
| `trend` | Track health metrics over git history at regular intervals |

### Coverage & Testing
| Subcommand | Description |
|------------|-------------|
| `test-gaps` | Untested public functions ranked by risk |

### Churn & Coupling
| Subcommand | Description |
|------------|-------------|
| `coupling-clusters` | Change-clusters: connected components of coupled files |
| `activity` | Cross-repo activity over time |
| `repo-coupling` | Analyze cross-repo coupling |

### Dependencies & Structure
| Subcommand | Description |
|------------|-------------|
| `architecture` | Codebase architecture: coupling, cycles, dependencies |
| `graph` | Graph-theoretic properties of the dependency graph (`--on modules\|symbols\|types`) |
| `call-graph` | Show callers and/or callees of a symbol (`--callers`, `--callees`) |
| `trace` | Trace value provenance for a symbol |
| `dependents` | Reverse-dependency closure: who depends on this file/symbol? |

### Documentation
| Subcommand | Description |
|------------|-------------|
| `docs` | Documentation coverage |

### Cross-cutting
| Subcommand | Description |
|------------|-------------|
| `security` | Security vulnerability patterns |
| `skeleton-diff` | Structural changes between a base ref and HEAD |
| `provenance` | Git blame → session mapping + code relations |

### Trend helpers
| Subcommand | Description |
|------------|-------------|
| `complexity-trend` | Complexity trend over git history |
| `length-trend` | Function length trend over git history |
| `density-trend` | Information density trend over git history |
| `test-ratio-trend` | Test ratio trend over git history |

## Examples

```bash
# Quick health check
normalize analyze

# Security scan
normalize analyze security

# Trace a symbol's data flow
normalize analyze trace parse_config

# Call graph
normalize analyze call-graph handle_request --callers
normalize analyze call-graph handle_request --callees

# Dependency graph analysis
normalize analyze graph                              # Module-level graph
normalize analyze graph --on symbols                 # Symbol-level graph
normalize analyze graph --on types                   # Type dependency graph

# Ranked lists are now under normalize rank:
normalize rank complexity                 # cyclomatic complexity per function
normalize rank hotspots                   # churn × complexity hotspots
normalize rank duplicates                 # code duplicates
normalize rank coupling                   # temporal coupling
```

## Options

### Global
- `-r, --root <PATH>` - Root directory
- `--json` - Output as JSON
- `--jsonl` - Output one JSON object per line
- `--jq <EXPR>` - Filter JSON with jq
- `--pretty` - Human-friendly output
- `--compact` - Compact output without colors
- `--input-schema` - Print JSON Schema of input parameters
- `--output-schema` - Print JSON Schema of return type
- `--params-json <JSON>` - Provide all parameters as a JSON object

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
