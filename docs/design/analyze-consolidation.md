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

**2b. ~~`coverage` family~~** — REVERTED. The three views (test-ratio, test-gaps, budget) had no shared data shape. Split back to separate commands: `test-ratio`, `test-gaps`, `budget`.

**2c. ~~`density` family~~** — NOT MERGING. `uniqueness` has 8 unique params. Keep `density`, `uniqueness`, `ceremony` separate.

**2d. ~~`churn` family~~** — REVERTED. The three views (coupling, coupling-clusters, hotspots) had no shared data shape. Split back to separate commands: `coupling`, `coupling-clusters`, `hotspots`.

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

**3. `graph` — pure graph-theoretic properties of the dependency graph (NOT traversal queries)**

`normalize analyze graph` (and any future `normalize graph`) is reserved for fully general graph theory: SCCs, bridges, diamond dependencies, transitive edges, dead nodes, graph density. These algorithms apply to any graph regardless of what nodes/edges represent.

`call-graph`, `trace`, `dependents` are **index traversal queries**, not graph theory. They ask "show me a specific path starting from this symbol/file" — normalize-specific lookups. They belong in `analyze`, not under `graph`.

This scope is **closed** — new graph-theoretic properties can be added as flags/output fields on the existing `graph` command, not as new commands. The `--on modules|symbols|types` flag already generalizes over node types.

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
| `graph` | Closed — pure graph theory only | No new commands; add fields to existing | Low (already correct) |
| `check` | Open — → rules engine | Already happening | Low (already have `rules`) |
| `churn` | Closed | Done | — |
| `coverage` | Closed | Done | — |
| Composites | Closed | Not worth merging (param divergence) | — |

#### What this means for the CLI

The target isn't "merge commands with compatible params." It's:

1. **`rank`**: Register metrics as a pluggable catalog. `normalize analyze rank complexity`, `normalize analyze rank density`, etc. Or keep short names (`complexity`, `density`) but backed by a shared `rank` infrastructure that gives all of them `--trend`, `--diff`, cross-repo support for free.

2. **`similar`**: One command with scope + mode flags. Delete 7 commands, add 1.

3. **`graph`**: Already correct scope — pure graph theory only. `call-graph`, `trace`, `dependents` are index traversal queries, not graph theory; they stay in `analyze`. No merging needed here.

4. **`check`**: Migrate hardcoded checks to the rules engine over time. No command-level change needed.

This would take 44 → ~20 commands, and more importantly, make the *extension model* obvious: adding a new metric is "register a scorer", not "add a command + args + dispatch + snapshot test."

## Implementation Strategy

**Enum wrappers are not real unification.** `CoverageOutput` and `CouplingOutput` were enum wrappers that reduced CLI entry points without unifying the data model. Each variant was still its own report struct with its own rendering. These have been reverted to separate commands (`test-ratio`/`test-gaps`/`budget` and `coupling`/`coupling-clusters`/`hotspots`).

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
| After `duplicates` unification (5 → 1, clusters absorbed) | 45 | -5 |
| After `fragments` absorbs `patterns` | 44 | -1 |
| After `check` unification (refs + stale + examples) | 42 | -2 |
| After `coverage`/`churn` enum reverts (+4 commands, -2 wrappers) | 42 | ±0 |
| After `dependents` absorbs `impact` | 41 | -1 |
| New commands added (trend variants, cross-repo, provenance) | 44 | +3 |
| After `graph` consolidation (NOT DOING — see design) | 44 | 0 |
| After further `check` → rules migration (future) | ~41 | ~-3 |

The goal isn't minimizing count for its own sake — it's making the mental model learnable and the extension model obvious.

## Implementation Progress

### Done

**`coverage`/`churn` enum wrappers** — REVERTED:
- `CoverageOutput` and `CouplingOutput` wrapped unrelated report types in enums
- No shared data shape existed between inner reports
- Split back to separate commands: `test-ratio`, `test-gaps`, `budget`, `coupling`, `coupling-clusters`, `hotspots`
- Enum wrapper files deleted

### Not merging (by design)

**`health`, `module-health`, `cross-repo-health`**: Different parameter signatures (target vs limit+min_lines vs repos_dir) mean these are genuinely different commands, not views of one command. Forcing them under one method creates a god-function with mostly-unused params. Keep separate.

**`density`, `uniqueness`, `ceremony`**: `uniqueness` has 8 unique params. Merging creates a 15-flag method where most flags are irrelevant to 2 of 3 views. Keep separate.

### Pattern Learned

**Enum wrappers were a mistake.** `CoverageOutput`, `CouplingOutput` reduced CLI entry points but didn't unify the data model. Each variant was a separate report with separate rendering — just dispatch with extra steps. Reverted: no shared data shape existed, so they're now separate commands again.

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

**`check`** — unified `check-refs`, `stale-docs`, `check-examples` (now subsumed by `normalize rules run --engine native`):
- `normalize rules run --engine native` → run all native checks (refs, stale docs, stale summaries, examples)
- Shared `DiagnosticsReport` struct (not an enum wrapper — all checks produce the same `Issue` type)
- `DiagnosticsReport` in `normalize-output::diagnostics` — reusable by any issue-reporting command
- Old commands: `check-refs`, `stale-docs`, `check-examples` — deleted; `analyze check` also deleted
- Output format: `file:line:col: severity [rule_id] message` (standard diagnostic format)

**`dependents` absorbs `impact`** (Phase 3, 2026-03-09):
- `normalize analyze dependents <target>` — now positional; for modules shows blast radius with test coverage; for symbols/types shows flat list
- `impact` was a file-only command computing the same reverse-dependency BFS but without `--on` support
- `DependentsReport` in `normalize-graph` expanded: adds `direct`, `transitive`, `blast_radius`, `untested_paths` (populated for modules graph); `dependents` flat list used for symbols/types
- Blast-radius computation (BFS + fan-in + test path detection) moved into `commands/analyze/graph.rs::analyze_module_dependents`
- Old command: `impact` — deleted

**`rank` table infrastructure** (Phase 3, 2026-03-12):
- `RankEntry` trait + `Column`/`Align` types + `format_ranked_table()` in `normalize-analyze::ranked`
- Entry structs implement `RankEntry` to define column names/alignment and per-row values
- `format_ranked_table(title, entries, empty_message)` renders: title line, dynamic-width columns, separator, rows
- Migrated 8 commands: files, imports, ownership, docs, ceremony, surface, depth-map, layering
- Each saves ~20-40 lines of manual table rendering; columns now auto-size consistently
- Pretty (`format_pretty`) still uses manual ANSI-colored rendering — those stay per-command
- **Not a `RankedReport<E>` generic struct** — every rank command has domain-specific metadata (per-language breakdowns, stats structs, layer summaries) beyond just entries+stats. A shared table *helper* gives 80% of the value without forcing a god-struct.
- **Next steps:** migrate remaining rank commands, then generic `--diff`/`--trend` infrastructure that gives all rank commands temporal analysis for free
