# normalize analyze

Analyze codebase quality: health, security, docs, and architectural structure.

Commands that produce ranked lists have moved to [`normalize rank`](rank.md).
Time-series trend commands have moved to [`normalize trend`](trend.md).
Call-graph navigation (`call-graph`, `trace`) lives under [`normalize view`](../cli-design.md).
Dependency-graph analysis (`graph`, `dependents`, `import-path`) has moved to the top-level
[`normalize graph`](../cli-design.md) verb (was `view graph`/`dependents`/`import-path`; the
old `view` paths remain as hidden aliases for one release).

## Subcommands

### Health & Overview

The aggregate dashboards moved to the top-level [`normalize overview`](../cli-design.md) verb
(B11): `analyze health`→`overview`, `analyze all`→`overview --full`, `analyze summary`→`overview
summary`, `analyze cross-repo-health`→`overview cross-repo-health`. Old `analyze` paths remain as
hidden aliases for one release, and a bare `normalize analyze <target>` still routes to the health
dashboard.

Git-history analysis (`coupling-clusters`, `activity`, `repo-coupling`) moved to the top-level
[`normalize history`](../cli-design.md) verb (owned by `normalize-git-history`, B9); old paths
remain as hidden aliases for one release.

Architecture analysis (`architecture`, `layering`, `depth-map`) has moved to the top-level
[`normalize architecture`](../cli-design.md) verb (was `analyze architecture`/`rank layering`/
`rank depth-map`; the old paths remain as hidden aliases for one release).

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
# Quick health check (dashboards are under normalize overview):
normalize overview
normalize overview --full                 # run all analysis passes

# Security scan
normalize analyze security

# Architecture analysis is under normalize architecture:
normalize architecture                    # coupling, hubs, layer flows
normalize architecture layering           # import-direction compliance
normalize architecture depth-map          # dependency depth + ripple risk

# Ranked lists are now under normalize rank:
normalize rank complexity                 # cyclomatic complexity per function
normalize rank hotspots                   # churn × complexity hotspots
normalize similarity                      # code duplicates (was rank duplicates)
normalize rank coupling                   # temporal coupling
normalize rank length                     # longest functions
normalize rank test-gaps                  # untested public functions

# Trend charts are now under normalize trend:
normalize trend complexity                # complexity trend over git history
normalize trend length                    # function length trend
normalize trend test-ratio                # test ratio trend

# Call-graph navigation is under normalize view:
normalize view referenced-by MyFunction   # callers of a symbol
normalize view references MyFunction      # callees of a symbol

# Dependency-graph analysis is under normalize graph:
normalize graph                           # module dependency graph
normalize graph dependents src/lib.rs     # what depends on this file
normalize graph import-path a.rs b.rs     # shortest import chain
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
