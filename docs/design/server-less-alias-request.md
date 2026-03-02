# Server-less Feature Request: CLI Aliases and Scoped Parameters

Status: **problem description** — to be shared with server-less for implementation.

## Context

normalize has 49 `analyze` subcommands. We're consolidating them into families — e.g. `coverage` unifies `test-ratio`, `test-gaps`, and `budget`. The consolidation uses an enum wrapper pattern:

```rust
#[derive(Serialize, schemars::JsonSchema)]
#[serde(tag = "view")]
enum CoverageOutput {
    Ratio(TestRatioReport),
    Gaps(TestGapsReport),
    Budget(BudgetReport),
}

#[cli(display_with = "display_coverage")]
pub fn coverage(&self, gaps: bool, budget: bool, ...) -> Result<CoverageOutput, String> {
    if gaps { Ok(CoverageOutput::Gaps(analyze_test_gaps(...))) }
    else if budget { Ok(CoverageOutput::Budget(analyze_budget(...))) }
    else { Ok(CoverageOutput::Ratio(analyze_test_ratio(...))) }
}
```

This works when views share most parameters. It breaks down in two specific cases that need server-less support.

## Problem 1: Command Aliases (backward compatibility)

When we merge `test-ratio`, `test-gaps`, and `budget` into `coverage`, the old names should still work as aliases:

```
normalize analyze coverage          # new canonical name
normalize analyze test-ratio        # alias → runs coverage (ratio view)
normalize analyze test-gaps         # alias → runs coverage --gaps
normalize analyze budget            # alias → runs coverage --budget
```

Currently there's no way to express this. The old commands remain as separate methods in the service, creating maintenance duplication.

### Proposed attribute

```rust
#[cli(display_with = "display_coverage")]
#[cli(alias = "test-ratio")]                              // alias for default view
#[cli(alias = "test-gaps", implies = ["gaps"])]           // alias that sets flags
#[cli(alias = "budget", implies = ["budget"])]            // alias that sets flags
pub fn coverage(&self, gaps: bool, budget: bool, ...) -> Result<CoverageOutput, String> { ... }
```

When `normalize analyze test-gaps` is invoked, it dispatches to `coverage()` with `gaps = true`.

**Simpler alternative** (if `implies` is too complex): just support `#[cli(alias = "test-ratio")]` as a plain name alias — all args pass through identically. The user would type `normalize analyze test-ratio --gaps` which is weird but functional. The `implies` variant is much better UX.

### Requirements

- Multiple aliases per method
- Aliases should appear in `--help` output (either as separate entries or noted alongside the canonical name)
- `--input-schema` and `--output-schema` should work identically via alias
- `--params-json` should work via alias
- Optional: deprecation notice when invoked via alias (e.g. `hint: test-ratio is now coverage`)

## Problem 2: Scoped Parameters

Some families have views where parameter signatures diverge significantly. Example — the density family:

```rust
// density:       root, limit, worst
// uniqueness:    root, limit, similarity, min_lines, skeleton, include_trait_impls, clusters, exclude, only
// ceremony:      root, limit
```

If we merge into one method, the unified signature has all parameters, but 8 of them only apply to `--uniqueness`:

```rust
pub fn density(
    &self,
    // shared
    root: Option<String>, limit: Option<usize>, pretty: bool, compact: bool,
    // density-specific
    worst: Option<usize>,
    // uniqueness-specific (5+ params that don't apply to other views)
    uniqueness: bool,
    similarity: Option<f64>,
    min_lines: Option<usize>,
    skeleton: bool,
    include_trait_impls: bool,
    clusters: Option<usize>,
    exclude: Vec<String>,
    only: Vec<String>,
    // ceremony-specific
    ceremony: bool,
) -> Result<DensityOutput, String> { ... }
```

This creates a confusing CLI where `--skeleton` appears in `--help` even when you're not using `--uniqueness`. Users see 15 flags and don't know which apply to their use case.

### Proposed attribute

```rust
pub fn density(
    &self,
    root: Option<String>,
    limit: Option<usize>,
    #[param(scope = "density")] worst: Option<usize>,
    #[param(scope = "uniqueness")] uniqueness: bool,
    #[param(scope = "uniqueness")] similarity: Option<f64>,
    #[param(scope = "uniqueness")] min_lines: Option<usize>,
    #[param(scope = "uniqueness")] skeleton: bool,
    #[param(scope = "uniqueness")] include_trait_impls: bool,
    #[param(scope = "uniqueness")] clusters: Option<usize>,
    #[param(scope = "uniqueness")] exclude: Vec<String>,
    #[param(scope = "uniqueness")] only: Vec<String>,
    ceremony: bool,
    pretty: bool,
    compact: bool,
) -> Result<DensityOutput, String> { ... }
```

Scoped parameters are only shown in `--help` when the relevant alias is used:

```
$ normalize analyze density --help
  --worst <N>    Worst files to show (default: 10)
  --uniqueness   Show uniqueness analysis
  --ceremony     Show ceremony analysis

$ normalize analyze uniqueness --help    # via alias
  --similarity <F>         Similarity threshold (default: 0.80)
  --min-lines <N>          Min function lines (default: 5)
  --skeleton               Match on control-flow skeleton
  --include-trait-impls    Include same-name groups
  --clusters <N>           Top clusters to show (default: 10)
  --exclude <PATTERN>      Exclude paths
  --only <PATTERN>         Include only paths
```

This ties into aliases: `#[cli(alias = "uniqueness", implies = ["uniqueness"])]` activates the `uniqueness` scope, which surfaces those parameters.

### Simpler alternative

If scoped parameters are too complex, the alias-with-implies feature alone would solve the backward compatibility problem. The parameter explosion in `--help` is a UX issue but not a blocker — the Rust type system still ensures correctness.

## Problem 3: `#[cli(default)]` on Enum-Returning Methods

The `health` command is marked `#[cli(default)]` — running `normalize analyze` with no subcommand runs `health()`. Currently `health()` returns `AnalyzeReport`. If we merge `module-health` and `cross-repo-health` into it, it would return `HealthOutput` (an enum).

This should work today since `#[cli(default)]` doesn't care about the return type. But there's a parameter conflict:

```rust
// health: target (positional), exclude, only
// module-health: limit, min_lines
// cross-repo-health: repos_dir (required positional), repos_depth
```

With aliases:

```rust
#[cli(default, display_with = "display_health")]
#[cli(alias = "module-health", implies = ["by_module"])]
#[cli(alias = "cross-repo-health", implies = ["cross_repo"])]  // but repos_dir is required!
pub fn health(
    &self,
    target: Option<String>,
    by_module: bool,
    cross_repo: bool,
    repos_dir: Option<String>,     // required when cross_repo=true, unused otherwise
    repos_depth: Option<usize>,    // only for cross-repo
    limit: Option<usize>,          // only for module
    min_lines: Option<usize>,      // only for module
    exclude: Vec<String>,
    only: Vec<String>,
    pretty: bool,
    compact: bool,
) -> Result<HealthOutput, String> { ... }
```

The issue: `repos_dir` is semantically required for `cross-repo-health` but must be `Option<String>` in the unified signature. Validation moves from the type system to runtime. This is acceptable if the error message is clear, but worth noting.

### What the default interaction looks like

```
$ normalize analyze                     # → health (default view)
$ normalize analyze --by-module         # → module-health view
$ normalize analyze module-health       # → alias for --by-module
$ normalize analyze --cross-repo --repos-dir ~/git/org/
$ normalize analyze cross-repo-health ~/git/org/
```

The `#[cli(default)]` method now has all the parameters for all views. Parameters for non-active views are ignored. This is the same pattern that coverage/churn use, just with `#[cli(default)]` additionally.

## Priority

1. **Aliases with `implies`** — highest value, unblocks all family merges and provides backward compat
2. **Scoped parameters** — nice to have, improves `--help` UX but not a blocker
3. **Default + aliases interaction** — should work naturally once aliases work, just needs testing

## Examples of the Consolidation This Enables

### Current state (49 commands)
```
normalize analyze health
normalize analyze module-health
normalize analyze cross-repo-health
normalize analyze test-ratio
normalize analyze test-gaps
normalize analyze budget
normalize analyze density
normalize analyze uniqueness
normalize analyze ceremony
normalize analyze coupling
normalize analyze coupling-clusters
normalize analyze hotspots
```

### After consolidation (~41 commands)
```
normalize analyze health                          # default
normalize analyze health --by-module              # was module-health
normalize analyze health --cross-repo DIR         # was cross-repo-health
normalize analyze coverage                        # was test-ratio
normalize analyze coverage --gaps                 # was test-gaps
normalize analyze coverage --budget               # was budget
normalize analyze density                         # was density
normalize analyze density --uniqueness            # was uniqueness
normalize analyze density --ceremony              # was ceremony
normalize analyze churn                           # was coupling
normalize analyze churn --cluster                 # was coupling-clusters
normalize analyze churn --hotspots                # was hotspots
```

Old names all still work via aliases. Users who learned the old names aren't broken; users learning the tool see a cleaner taxonomy.

## How Server-less Benefits

This isn't normalize-specific. Any server-less consumer that evolves its API will want:

- **Alias with implies**: rename/consolidate commands without breaking callers
- **Scoped parameters**: keep `--help` focused when one method serves multiple purposes
- **Deprecation notices**: guide users to the new names

The alias feature also benefits the RPC/OpenAPI/MCP protocols — aliases could map to the same endpoint, enabling API evolution without version bumps.
