# normalize analyze

Analyze codebase quality: health, security, docs, and architectural structure.

Commands that produce ranked lists have moved to [`normalize rank`](rank.md).
Time-series trend commands have moved to [`normalize trend`](trend.md).
Graph navigation (`call-graph`, `trace`, `dependents`, `graph`) has moved to [`normalize view`](../cli-design.md).

## Subcommands

### Health & Overview
| Subcommand | Description |
|------------|-------------|
| `health` | File counts, complexity stats, large file warnings (default when no subcommand) |
| `summary` | Auto-generated single-page codebase overview |
| `all` | Run all analysis passes |

### Churn & Coupling
| Subcommand | Description |
|------------|-------------|
| `coupling-clusters` | Change-clusters: connected components of coupled files |
| `activity` | Cross-repo activity over time |
| `repo-coupling` | Analyze cross-repo coupling |
| `cross-repo-health` | Rank repos by tech debt (churn + complexity + coupling) |

### Dependencies & Structure
| Subcommand | Description |
|------------|-------------|
| `architecture` | Codebase architecture: coupling, cycles, hub modules |

### Documentation
| Subcommand | Description |
|------------|-------------|
| `docs` | Documentation coverage (public symbols with/without doc comments) |

### Cross-cutting
| Subcommand | Description |
|------------|-------------|
| `security` | Security vulnerability patterns |
| `skeleton-diff` | Structural changes between a base ref and HEAD |

## Examples

```bash
# Quick health check
normalize analyze

# Security scan
normalize analyze security

# Architecture analysis
normalize analyze architecture

# Ranked lists are now under normalize rank:
normalize rank complexity                 # cyclomatic complexity per function
normalize rank hotspots                   # churn × complexity hotspots
normalize rank duplicates                 # code duplicates
normalize rank coupling                   # temporal coupling
normalize rank length                     # longest functions
normalize rank test-gaps                  # untested public functions

# Trend charts are now under normalize trend:
normalize trend complexity                # complexity trend over git history
normalize trend length                    # function length trend
normalize trend test-ratio                # test ratio trend

# Graph navigation is now under normalize view:
normalize view referenced-by MyFunction   # callers of a symbol
normalize view references MyFunction      # callees of a symbol
normalize view graph src/lib.rs           # dependency graph
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
