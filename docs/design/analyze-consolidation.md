# Analyze Command Consolidation

Status: **design** — phased implementation in progress.

## Problem

`normalize analyze` has 49 subcommands. Each is a hardcoded point in a 4-dimensional space:

| Axis | Values |
|------|--------|
| **Metric** | complexity, length, density, ceremony, uniqueness, test-ratio, budget, health, duplicates, coupling, churn, ownership, surface, depth, layering, calls, docs, security |
| **Scope** | function, file, module, codebase, cross-repo |
| **Time** | snapshot (now), vs-ref (diff against a commit), over-history (trend) |
| **Shape** | score, ranked-list, grouped/clustered, graph/tree, diff |

For example:
- `coupling-clusters` = coupling × module × snapshot × grouped
- `skeleton-diff` = structure × codebase × vs-ref × diff
- `trend` = health × codebase × over-history × time-series
- `module-health` = health × module × snapshot × ranked-list

Users don't think in named intersections. They think: "show me coupling, but grouped" or "how has health changed over time?" The current model forces memorizing 49 names.

## Axis Decomposition

Full mapping of all 49 commands to their axis coordinates:

### Health & Scoring
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `health` | composite | codebase | now | score |
| `module-health` | composite | module | now | ranked-list |
| `cross-repo-health` | composite | cross-repo | now | ranked-list |
| `summary` | composite | codebase | now | report |
| `trend` | composite | codebase | over-history | time-series |
| `all` | composite | codebase | now | score (multi-section) |

### Complexity
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `complexity` | cyclomatic | function | now | ranked-list |
| `call-complexity` | reachable-cyclomatic | function | now | ranked-list |
| `length` | line-count | function | now | ranked-list |

### Duplicates & Similarity
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `duplicate-functions` | exact-hash | function | now | grouped |
| `duplicate-blocks` | exact-hash | block | now | grouped |
| `duplicate-types` | name+structure | type | now | grouped |
| `similar-functions` | minhash-lsh | function | now | pairs |
| `similar-blocks` | minhash-lsh | block | now | pairs |
| `clusters` | minhash-union-find | function | now | grouped |
| `patterns` | skeleton-minhash | function | now | grouped |

### Coverage & Testing
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `test-ratio` | test-lines/impl-lines | module | now | ranked-list |
| `test-gaps` | untested-public-fns | function | now | ranked-list |
| `budget` | line-purpose-breakdown | module | now | ranked-list |

### Information Density
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `density` | compression-ratio | module | now | ranked-list |
| `uniqueness` | structural-twin-ratio | module | now | ranked-list |
| `ceremony` | boilerplate-ratio | file | now | ranked-list |

### Coupling & Churn
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `coupling` | temporal-co-change | file-pair | now | pairs |
| `coupling-clusters` | temporal-co-change | file-group | now | grouped |
| `hotspots` | churn×complexity | file | now | ranked-list |

### Dependencies & Structure
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `imports` | fan-in | module | now | ranked-list |
| `depth-map` | dag-depth+ripple | module | now | ranked-list |
| `surface` | public-ratio+fan-in | module | now | ranked-list |
| `layering` | downward-compliance | module | now | ranked-list |
| `architecture` | coupling+cycles+hubs | codebase | now | report |
| `call-graph` | call-edges | symbol | now | tree |
| `callers` | reverse-call-edges | symbol | now | list |
| `callees` | forward-call-edges | symbol | now | list |
| `trace` | value-provenance | symbol | now | tree |
| `impact` | reverse-dep-closure | symbol | now | tree |

### Documentation
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `docs` | doc-coverage | file | now | ranked-list |
| `check-refs` | broken-refs | file | now | list |
| `stale-docs` | stale-docs | file | now | list |
| `check-examples` | missing-examples | file | now | list |

### Cross-cutting
| Command | Metric | Scope | Time | Shape |
|---------|--------|-------|------|-------|
| `skeleton-diff` | structure | codebase | vs-ref | diff |
| `provenance` | git-blame+sessions | file | now | graph |
| `security` | security-patterns | file | now | list |
| `ownership` | blame-concentration | file | now | ranked-list |
| `contributors` | commit-activity | cross-repo | now | ranked-list |
| `activity` | time-series-commits | cross-repo | over-history | time-series |
| `repo-coupling` | shared-contributors | cross-repo | now | pairs |
| `files` | line-count | file | now | ranked-list |
| `size` | loc-hierarchy | module | now | tree |

## Design: Composable Families

### Phase 1 — Cross-cutting modifiers (future)

Add `--trend` and `--diff <ref>` as universal modifiers on any scoring command. This is highest leverage but requires the most infrastructure (any command that returns a score needs to be runnable at arbitrary commits). Deferred until Phase 2 proves the family model works.

### Phase 2 — Merge obvious families (current focus)

Group commands that share implementation machinery under a single parent command with view flags. Old names become aliases (backward compat).

**2a. `health` family:**
```
normalize analyze health                     # was: health (default)
normalize analyze health --by-module         # was: module-health
normalize analyze health --cross-repo DIR    # was: cross-repo-health
```
Shared machinery: `HealthReport`, `score_breakdown()`, grade ladder.

**2b. `coverage` family:**
```
normalize analyze coverage                   # was: test-ratio (default)
normalize analyze coverage --gaps            # was: test-gaps
normalize analyze coverage --budget          # was: budget
```
All three measure test/coverage at module level.

**2c. `density` family:**
```
normalize analyze density                    # was: density (default)
normalize analyze density --uniqueness       # was: uniqueness
normalize analyze density --ceremony         # was: ceremony
```
All three are per-module information quality metrics.

**2d. `coupling` family:**
```
normalize analyze coupling                   # was: coupling (default)
normalize analyze coupling --cluster         # was: coupling-clusters
normalize analyze coupling --hotspots        # was: hotspots
```
All three are temporal co-change analyses from git history.

### Phase 3 — Evaluate further consolidation (future)

Candidates that need more design work:

- **`duplicates`**: 7 commands (duplicate-functions, duplicate-blocks, duplicate-types, similar-functions, similar-blocks, clusters, patterns). All use MinHash/LSH + union-find but differ in granularity, threshold, and output shape. May collapse to `duplicates --scope functions|blocks|types --mode exact|similar|cluster|patterns`.

- **`deps`**: 10 commands (imports, depth-map, surface, layering, architecture, call-graph, callers, callees, trace, impact). Split between file-level metrics (imports, depth-map, surface, layering) and symbol-level graph traversals (call-graph, callers, callees, trace, impact). Forcing all under one command risks a god-command. May split into `deps` (file-level) + `graph` (symbol-level).

- **`docs`**: 4 commands (docs, check-refs, stale-docs, check-examples). Small family, low urgency.

- **Cross-cutting `--trend` and `--diff`**: Any metric that produces a score could be trended over time (`--trend -n 5`) or diffed against a ref (`--diff main`). This replaces `trend` and `skeleton-diff` as standalone commands but requires infrastructure to run any analysis at an arbitrary commit.

## Implementation Strategy

Each Phase 2 merge follows this pattern:

1. Add a `view` or mode parameter to the parent command's service method
2. Dispatch to the appropriate analysis function based on the view
3. Return the existing report type (no report struct changes needed — each view returns its own type)
4. The old command names remain as aliases (server-less `#[cli(alias = "...")]`)
5. Update snapshot tests

The key constraint is that server-less `#[cli]` methods must return a single type. For families where each view returns a different report type, we'll need an enum wrapper. This is the same pattern used by `ViewResult` in the view service.

## Command Count

| Phase | Commands | Reduction |
|-------|----------|-----------|
| Current | 49 | — |
| After Phase 2 | 41 | -8 (4 families absorb 12 commands, net -8) |
| After Phase 3 (est.) | ~25 | ~-16 more |

The goal isn't minimizing count for its own sake — it's making the mental model learnable. 25 commands with clear families is better than 49 flat names where `module-health` and `health` feel unrelated.

## Implementation Progress

### Done

**`coverage`** — unifies `test-ratio`, `test-gaps`, `budget`:
- `normalize analyze coverage` → test-ratio (default)
- `normalize analyze coverage --gaps` → test-gaps
- `normalize analyze coverage --budget` → budget
- Enum: `CoverageOutput` in `commands/analyze/coverage.rs`
- Old commands remain for backward compatibility

**`churn`** — unifies `coupling`, `coupling-clusters`, `hotspots`:
- `normalize analyze churn` → coupling pairs (default)
- `normalize analyze churn --cluster` → coupling-clusters
- `normalize analyze churn --hotspots` → hotspots
- Enum: `CouplingOutput` in `commands/analyze/coupling_views.rs`
- Old commands remain for backward compatibility

### Deferred

**`health`**: Existing `health` is the default command (`#[cli(default)]`), changing its return type is too disruptive. `module-health` and `cross-repo-health` have very different param signatures (target vs limit+min_lines vs repos_dir). Needs server-less alias support or a design rethink.

**`density`**: `uniqueness` has 8 extra params (similarity, min_lines, skeleton, include_trait_impls, clusters, exclude, only) that make a unified method unwieldy. Needs a way to scope params to specific views.

### Pattern Learned

Enum wrapper (`CoverageOutput`, `CouplingOutput`) + `OutputFormatter` delegation works well when:
- Views share most parameters (root, limit, exclude, only)
- View-specific params are few and can be `Option`
- No `#[cli(default)]` collision

It doesn't work well when:
- Parameter signatures diverge significantly (health family)
- One view has 5+ unique params (density/uniqueness)
- The parent command is already the CLI default
