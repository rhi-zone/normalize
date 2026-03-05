# Analyze Command Consolidation

Status: **design** — phased implementation in progress.

## Problem

`normalize analyze` has 50 subcommands. Each is a hardcoded point in a 4-dimensional space:

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
| `duplicates` | exact-hash/minhash/union-find | function/block | now | grouped/pairs |
| `duplicate-types` | name+structure | type | now | grouped |
| `fragments` | subtree-hash/minhash/skeleton | any/function/block | now | grouped |

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

### Phase 2 — Merge families with compatible parameters

Merge commands that share most parameters into a single command with view flags. Delete old names (no aliases — we're at v0.1.0).

**Only merge when parameter signatures are compatible.** If one view has 5+ unique params, or params are semantically different (target path vs repos directory), they're different commands.

**2a. ~~`health` family~~** — NOT MERGING. `health`, `module-health`, `cross-repo-health` have divergent params (target vs limit+min_lines vs repos_dir). Keep separate.

**2b. `coverage` family** (done):
```
normalize analyze coverage                   # was: test-ratio (default)
normalize analyze coverage --gaps            # was: test-gaps
normalize analyze coverage --budget          # was: budget
```

**2c. ~~`density` family~~** — NOT MERGING. `uniqueness` has 8 unique params. Keep `density`, `uniqueness`, `ceremony` separate.

**2d. `churn` family** (done):
```
normalize analyze churn                      # was: coupling (default)
normalize analyze churn --cluster            # was: coupling-clusters
normalize analyze churn --hotspots           # was: hotspots
```
All three are temporal co-change analyses from git history.

### Phase 3 — Concept graph decomposition

Instead of asking "which commands can merge?", ask: "what are the underlying concepts, and which generalize?"

#### Entities (nodes in the concept graph)

```
symbol (function, type, trait)
file
module (directory)
commit
author
repo
```

#### Properties (computed per entity)

```
complexity(symbol)        → cyclomatic count
lines(symbol|file)        → line count
is_test(symbol|file)      → bool
is_public(symbol)         → bool
has_doc(symbol)           → bool
is_boilerplate(symbol)    → trait impl / interface
hash(symbol|block)        → exact content hash
minhash(symbol|block)     → similarity signature
skeleton(symbol)          → control-flow shape
compression_ratio(module) → information density
```

#### Relations (edges)

```
contains(file, symbol)
contains(module, file)
calls(symbol, symbol)
imports(file, file)
changed_in(file, commit)
authored_by(commit, author)
similar_to(symbol, symbol, score)
duplicate_of(symbol, symbol)
tests(symbol, symbol)
```

#### Derived metrics (compositions)

```
churn(file)       = count(changed_in(file, _))
coupling(f1, f2)  = |commits(f1) ∩ commits(f2)|
hotspot(file)     = churn(file) × avg_complexity(file)
ownership(file)   = concentration(authors(blame(file)))
fan_in(module)    = count(imports(_, module))
depth(module)     = max_path(import_dag, module)
test_ratio(mod)   = lines(tests_in(mod)) / lines(impl_in(mod))
uniqueness(mod)   = 1 - fraction_with_similar_to(mod)
```

#### Four extensible patterns

All 45 commands decompose into 4 extensible patterns + specific features + composites:

**1. `rank <metric>` — score entities, show worst-first (open set)**

New metrics plug in naturally. Today this covers ~15 commands:
complexity, length, files, size, density, uniqueness, ceremony, ownership, imports, depth-map, surface, layering, docs.

All share the same shape: compute a scalar per entity, sort, show top N. The metric and entity scope differ but the machinery is identical.

**2. `similar` — find structurally alike code units (open set)**

Today: duplicates (5 modes via `--mode`/`--scope`), duplicate-types, fragments. All ask "which code units look alike?"

`duplicates` already unified: `--mode exact|similar|clusters --scope functions|blocks`. `patterns` absorbed into `fragments` (use `--scope functions --skeleton --similarity 0.7 --min-members 3`). Could further absorb under a broader `similar` command.

**3. `graph <symbol>` — walk relations from a starting point (open set)**

Today: call-graph, callers, callees, trace, impact. All walk the call/dependency graph from a symbol. Direction (up/down/both) and depth (direct/transitive) differ.

Could become: `graph <symbol> [--callers|--callees|--both] [--transitive] [--impact]`

**4. `check` — find violations / scan for problems (→ subsumes into rules engine)**

Today: docs, check-refs, stale-docs, check-examples, security. All scan files for violations of some predicate. Many could eventually become tree-sitter rules rather than hardcoded commands. The `rules` engine already does this for user-defined patterns.

#### Specific features (closed set — don't generalize)

- `churn` — temporal analysis from git history (already unified, 3 views)
- `coverage` — test coverage analysis (already unified, 3 views)
- `skeleton-diff` — structural comparison vs a ref
- `provenance` — blame → session mapping
- `architecture` — composite coupling+cycles+hubs report

#### Composites (presentation, not concepts)

- `health`, `module-health`, `cross-repo-health`, `summary`, `all`, `trend`
- These run other concepts and aggregate. They're dashboards.
- Cross-repo variants (`cross-repo-health`, `activity`, `contributors`, `repo-coupling`) are the same concepts applied at a wider scope.

#### Extensibility verdict

| Pattern | Open/Closed | Generalizes? | Priority |
|---------|-------------|-------------|----------|
| `rank <metric>` | Open — new metrics frequently | Yes, highest leverage | High |
| `similar` | Open — new scopes/methods | Yes, 7 → 1 | High |
| `graph` | Open — new relation types | Yes, 5 → 1 | Medium |
| `check` | Open — → rules engine | Already happening | Low (already have `rules`) |
| `churn` | Closed | Done | — |
| `coverage` | Closed | Done | — |
| Composites | Closed | Not worth merging (param divergence) | — |

#### What this means for the CLI

The target isn't "merge commands with compatible params." It's:

1. **`rank`**: Register metrics as a pluggable catalog. `normalize analyze rank complexity`, `normalize analyze rank density`, etc. Or keep short names (`complexity`, `density`) but backed by a shared `rank` infrastructure that gives all of them `--trend`, `--diff`, cross-repo support for free.

2. **`similar`**: One command with scope + mode flags. Delete 7 commands, add 1.

3. **`graph`**: One command with direction + depth flags. Delete 5 commands, add 1.

4. **`check`**: Migrate hardcoded checks to the rules engine over time. No command-level change needed.

This would take 44 → ~20 commands, and more importantly, make the *extension model* obvious: adding a new metric is "register a scorer", not "add a command + args + dispatch + snapshot test."

## Implementation Strategy

**Enum wrappers are not real unification.** `CoverageOutput`, `CouplingOutput` wrap N report types in an enum with `OutputFormatter` delegation. This reduces CLI entry points but doesn't unify the data model — each variant is still its own report struct with its own rendering. Real consolidation means one report struct that all modes populate, with shared fields and shared rendering. Revisit existing enum wrappers.

For pattern #4 (`check`), the right unification is the **diagnostic model**: `check-refs`, `stale-docs`, `check-examples`, `security` all produce "list of issues found in files." These should share a common diagnostic output format and ideally migrate into the rules engine over time.

Each merge follows this pattern:

1. Identify the shared data shape across modes
2. Design a single report struct (not an enum) with optional mode-specific fields
3. Implement `OutputFormatter` once, with mode-aware rendering
4. **Delete the old commands** — no aliases, no backward compat at v0.1.0
5. Update snapshot tests

## Command Count

| Phase | Commands | Reduction |
|-------|----------|-----------|
| Start | 50 | — |
| After Phase 2 (coverage + churn merged, old deleted) | 44 | -6 |
| After `duplicates` unification (5 → 1, clusters absorbed) | 39 | -5 |
| After `fragments` absorbs `patterns` | 38 | -1 |
| After `check` unification (refs + stale + examples) | 36 | -2 |
| After `graph` consolidation | 31 | -4 |
| After further `check` → rules migration | ~28 | ~-3 |

The goal isn't minimizing count for its own sake — it's making the mental model learnable and the extension model obvious.

## Implementation Progress

### Done

**`coverage`** — unifies `test-ratio`, `test-gaps`, `budget`:
- `normalize analyze coverage` → test-ratio (default)
- `normalize analyze coverage --gaps` → test-gaps
- `normalize analyze coverage --budget` → budget
- Enum: `CoverageOutput` in `commands/analyze/coverage.rs`
- Old commands: `test-ratio`, `test-gaps`, `budget` — delete once coverage is proven

**`churn`** — unifies `coupling`, `coupling-clusters`, `hotspots`:
- `normalize analyze churn` → coupling pairs (default)
- `normalize analyze churn --cluster` → coupling-clusters
- `normalize analyze churn --hotspots` → hotspots
- Enum: `CouplingOutput` in `commands/analyze/coupling_views.rs`
- Old commands: `coupling`, `coupling-clusters`, `hotspots` — delete once churn is proven

### Not merging (by design)

**`health`, `module-health`, `cross-repo-health`**: Different parameter signatures (target vs limit+min_lines vs repos_dir) mean these are genuinely different commands, not views of one command. Forcing them under one method creates a god-function with mostly-unused params. Keep separate.

**`density`, `uniqueness`, `ceremony`**: `uniqueness` has 8 unique params. Merging creates a 15-flag method where most flags are irrelevant to 2 of 3 views. Keep separate.

### Pattern Learned

**Enum wrappers were a mistake.** `CoverageOutput`, `CouplingOutput` reduce CLI entry points but don't unify the data model. Each variant is still a separate report with separate rendering — it's just dispatch with extra steps. These should be revisited: either find the shared data shape and use a single struct, or accept they're different commands.

**Single struct with shared fields is real unification** (`DuplicatesReport`):
- All modes share the same output shape (groups of code locations)
- Mode differences are which optional fields are populated
- `serde(skip_serializing_if = "Option::is_none")` keeps JSON clean per mode
- One `OutputFormatter` impl with mode-aware rendering

It doesn't work when parameter signatures diverge — that means they're different commands, not different views. Don't force a merge with aliases or god-functions; accept that separate commands are the right design.

**No aliases.** We're at v0.1.0 with no external users depending on old names. Old commands get deleted, not aliased. Aliases double surface area and never get cleaned up.

**`duplicates`** — unifies `duplicate-functions`, `duplicate-blocks`, `similar-functions`, `similar-blocks`, `clusters`:
- `normalize analyze duplicates` → exact duplicate functions (default)
- `normalize analyze duplicates --scope blocks` → exact duplicate blocks
- `normalize analyze duplicates --mode similar` → similar functions
- `normalize analyze duplicates --mode similar --scope blocks` → similar blocks
- `normalize analyze duplicates --mode clusters` → function clusters (was standalone `clusters` command)
- Single `DuplicatesReport` struct with mode-aware `OutputFormatter`
- Old commands: `duplicate-functions`, `duplicate-blocks`, `similar-functions`, `similar-blocks`, `clusters` — deleted

**`fragments` absorbs `patterns`**:
- `normalize analyze fragments --scope functions --skeleton --similarity 0.7 --min-members 3` → was `patterns`
- Added `--min-members` flag, `avg_similarity` per cluster (fuzzy mode), `unclustered_count` in report
- Old command: `patterns` — deleted

**`check`** — unifies `check-refs`, `stale-docs`, `check-examples`:
- `normalize analyze check` → run all documentation checks (default)
- `normalize analyze check --refs` → broken documentation references
- `normalize analyze check --stale` → stale documentation
- `normalize analyze check --examples` → missing example markers
- Shared `DiagnosticsReport` struct (not an enum wrapper — all checks produce the same `Issue` type)
- `DiagnosticsReport` in `normalize-output::diagnostics` — reusable by any issue-reporting command
- Old commands: `check-refs`, `stale-docs`, `check-examples` — deleted
- Output format: `file:line:col: severity [rule_id] message` (standard diagnostic format)
