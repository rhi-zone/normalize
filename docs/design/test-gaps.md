# Test Gaps Analysis

Identify public functions and methods with no direct unit test.

## Problem

Code coverage tools (tarpaulin, llvm-cov) measure which lines execute during tests. They answer "was this code reached?" but not "does this function have a dedicated test?"

A function can show 100% line coverage while having zero direct tests - it's exercised only as a side effect of testing something else. This is fragile: if the caller changes, test coverage silently disappears. The function had no contract of its own.

We want a **static** analysis (no test execution required) that answers: for each public function, does any test function directly call it?

## Key Insight

The gap between 0 and 1 direct test callers is categorical, not quantitative. Zero means "no contract" - the function's behavior is only accidentally verified. One or more means "someone wrote a test for this." The quality of those tests is a separate concern (mutation testing, property testing, etc.) - but the existence of at least one direct test is the minimum bar.

"Indirectly tested" is a false comfort. If `test_process_order()` calls `validate_address()` internally, and someone later refactors `process_order()` to skip validation, there's no test failure for `validate_address()`. It was never the subject of a test - just a bystander.

## Solution

New subcommand: `normalize analyze test-gaps`

Uses the existing call graph index to find public functions with zero callers from test context. Sorts results by risk to surface the most dangerous gaps first.

### CLI Interface

```bash
# Find all functions with no direct test caller
normalize analyze test-gaps

# Scope to a directory
normalize analyze test-gaps src/commands/

# Scope to a specific file
normalize analyze test-gaps src/index.rs

# Show all functions (including tested ones), sorted by test calls ascending
normalize analyze test-gaps --all

# Only functions above a risk threshold
normalize analyze test-gaps --min-risk 10

# Limit output
normalize analyze test-gaps --limit 20

# Allow a known-untested function
normalize analyze test-gaps --allow src/main.rs:main --reason "Entry point, integration tested"

# SARIF output for IDE integration
normalize analyze test-gaps --sarif

# JSON for scripting
normalize analyze test-gaps --json
```

### Output

Default text output (sorted by risk descending among untested functions):

```
Test Gaps: 23 of 142 public functions have no direct test

 Risk  Function                          File                  Complexity  Callers  LOC
 ────  ────────────────────────────────  ────────────────────  ──────────  ───────  ───
 47.2  propagate_constraints             src/solver.rs:145     18          12       94
 31.5  resolve_symbol_recursive          src/index.rs:302      14          8        67
 28.0  parse_pattern                     src/rules/parser.rs   12          11       53
  9.3  format_sarif_result               src/sarif.rs:45       4           3        28
  3.2  default_config                    src/config.rs:12      1           6        8
  ...

Allowed: 2 functions (src/main.rs:main, src/lib.rs:run_cli)
```

Summary line gives the ratio. Table shows the gap sorted by risk. Allowed functions are listed but excluded from the count.

### `--all` Output

When `--all` is passed, show every function with test-call count:

```
Test Gaps: 23 of 142 public functions have no direct test

 Tests  Risk  Function                     File                  Complexity  Callers  LOC
 ─────  ────  ───────────────────────────  ────────────────────  ──────────  ───────  ───
     0  47.2  propagate_constraints        src/solver.rs:145     18          12       94
     0  31.5  resolve_symbol_recursive     src/index.rs:302      14          8        67
     ...
     1   -    add_tile                     src/tileset.rs:34     3           5        12
     3   -    parse_direction              src/direction.rs:8    2           8        6
     7   -    new                          src/solver.rs:20      1           15       4
```

Functions with 1+ test callers have no risk score (the binary threshold is met).

## Risk Scoring

Risk quantifies "how dangerous is it that this function has no test?" It combines three signals:

```
risk = complexity * ln(callers + 1) * ln(loc + 1)
```

| Signal | Why | Source |
|--------|-----|--------|
| Cyclomatic complexity | More branches = more ways to fail | `analyze complexity` (existing) |
| Caller count | More callers = larger blast radius when it breaks | Call graph index (existing) |
| Lines of code | More code = more surface area for bugs | Symbol index (existing) |

Logarithmic scaling on callers and LOC prevents one extreme value from dominating. A function with complexity 15 and 3 callers is riskier than one with complexity 2 and 50 callers - the complexity signal should dominate.

**Why not just complexity?** A complex function called by nothing is dead code (a different analysis). A simple function called by everything is low-risk (one-liner getters). Risk captures the intersection: complex, widely-used, untested.

## Detecting Test Context

A function is a "test caller" if it exists in test context. This is language-specific:

| Language | Test context indicators |
|----------|----------------------|
| Rust | `#[test]` attribute, `#[cfg(test)]` module, `tests/` directory |
| Python | `test_` prefix, `tests/` or `test/` directory, `unittest.TestCase` subclass |
| Go | `_test.go` file suffix, `Test` function prefix |
| JavaScript/TypeScript | `*.test.ts`, `*.spec.ts`, `__tests__/` directory, `describe`/`it`/`test` blocks |
| Java | `@Test` annotation, `*Test.java` file suffix, `src/test/` directory |
| Ruby | `*_test.rb`, `*_spec.rb`, `spec/` directory |
| C# | `[Test]`, `[Fact]`, `[Theory]` attributes, `*.Tests` project |
| PHP | `*Test.php`, `tests/` directory |

The file index already stores symbol locations. The detection heuristic needs to classify each symbol as test-or-not based on these language-specific rules. This classification should live in `normalize-languages` alongside other language-specific knowledge.

### Transitive test callers

A helper function called only from test context is itself a test helper, not production code. If `setup_test_db()` calls `reset_schema()`, and `setup_test_db` is only ever called from `#[test]` functions, then `reset_schema` has a test caller (via a test helper).

Rule: a function is "test-called" if any of its **direct** callers are in test context. We do **not** chase transitive callers arbitrarily deep - that would defeat the purpose (everything becomes "indirectly tested" again). But test helpers one level up are legitimate test callers because they exist solely to support tests.

Implementation: mark all functions in test context as "test functions." Then for each public function, check if any direct caller is a test function. One level of indirection captures test helpers without collapsing back to "everything is tested."

## OOTB Heuristics

The tool should be correct by default without user configuration. These heuristics reduce false positives:

### Functions excluded from analysis

These are never flagged, regardless of test status:

| Exclusion | Rationale | Detection |
|-----------|-----------|-----------|
| `main()` / entry points | Integration tested, not unit tested | Function name + top-level scope |
| Derived trait implementations | `Default::default()`, `Clone::clone()`, etc. | AST: `#[derive(...)]` on parent type |
| Generated code | Build scripts, proc macros, protobuf | File path patterns (`*.generated.*`, `build/`, etc.) |
| Test code itself | Tests don't need tests | Already classified as test context |
| Private functions | Not part of public API contract | Visibility: `pub` filter (language-specific) |

### Functions de-prioritized (lower risk score)

These are still reported but with reduced risk:

| Pattern | Rationale | Detection |
|---------|-----------|-----------|
| Trait method sugar that delegates to an op | Testing the op covers the sugar | Body is single expression calling another function |
| Constructors (`new`, `default`, `from`) | Low complexity, often trivially correct | Function name pattern |
| Getters/setters | Trivial body | Complexity = 1, LOC <= 3, returns field or sets field |
| `Display`/`Debug` implementations | Formatting, not logic | Trait impl name |

De-prioritization multiplies risk by 0.1, pushing these to the bottom of the list without hiding them entirely.

### What we explicitly do NOT exclude

These might seem like candidates for exclusion but are intentionally kept:

| Keep | Why |
|------|-----|
| Functions with high caller count | Popular functions are MORE important to test, not less |
| Builder methods (`with_x`, `set_x`) | They can have validation logic; don't assume they're trivial |
| Error handling functions | Often complex, often undertested |
| Functions in `impl` blocks | Methods are functions; they need tests |

## Implementation

### Prerequisites

Requires call graph index. If not available:
```
$ normalize analyze test-gaps
Error: Call graph not indexed. Run: normalize facts rebuild
```

### Data Flow

```
Symbol Index ──────────┐
                       │
Call Graph Index ──────┼──→ Classify test context
                       │         │
Complexity Data ───────┤         ▼
                       │    For each public function:
                       │    - Count direct test callers
                       │    - Compute risk score
                       │    - Apply exclusions/de-prioritization
                       │         │
                       │         ▼
                       └──→ Sort, format, output
```

### Modules

| File | Responsibility |
|------|---------------|
| `commands/analyze/test_gaps.rs` | Subcommand entry point, arg parsing, output |
| `analyze/test_gaps.rs` | Core analysis: classify, count, score |
| Language-specific additions in `normalize-languages` | Test context detection rules per language |

### Database Queries

The analysis runs these queries against the existing index:

1. **All public functions**: `SELECT * FROM symbols WHERE kind IN ('function', 'method') AND visibility = 'public'`
2. **Test classification**: For each symbol, check if file/module/attributes indicate test context
3. **Direct callers per function**: `SELECT caller_symbol FROM calls WHERE callee_name = ?`
4. **Complexity per function**: computed on-the-fly or cached from `analyze complexity`

### Allow List

Stored in `.normalize/test-gaps-allow`:
```
# Entry points
src/main.rs:main  # Entry point, integration tested
src/lib.rs:run_cli  # CLI entry, integration tested

# FFI boundaries
src/ffi.rs:*  # Tested via integration tests in tests/
```

Supports exact symbols and glob patterns per file. Follows the same pattern as `complexity-allow`, `length-allow`, etc.

## Integration with `analyze all`

`test-gaps` should be included in `analyze all` with a weight that reflects its importance:

```rust
pub struct AnalyzeWeights {
    // ... existing weights ...
    pub test_gaps: f64,  // default 1.0
}
```

The health score contribution: `(tested_count / total_count) * weight`. A codebase with 80% directly-tested public functions gets 0.8 * 1.0 = 0.8 contribution from this metric.

## Non-Goals

- **Mutation testing** - That's a separate tool (cargo-mutants). We answer "is there a test?" not "is the test good?"
- **Coverage measurement** - We don't run tests. This is static analysis only.
- **Test generation** - We identify gaps, we don't fill them. (But the output is ideal input for an LLM agent that writes tests.)
- **Private function analysis** - Private functions are implementation details. Test the public API; refactoring internals shouldn't break tests.

## Future Extensions

- **Trend tracking**: Compare test-gaps across git commits. "This PR adds 5 public functions and 0 tests" as a CI gate.
- **Test quality signal**: Combine with mutation testing results. A function with 3 tests that all survive mutations is worse than one with 1 test that catches mutations.
- **Suggested test skeletons**: For each untested function, generate a test signature with the right imports and parameter types. Not the test body (that requires understanding intent), just the boilerplate.
