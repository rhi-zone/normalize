# Diagnosis: `normalize sessions stats` pretty output broken

**Date:** 2026-06-28
**Status:** Pre-existing bug made visible by commit `2351a201`

---

## What is broken

`normalize sessions stats --pretty` produces identical output to the default text mode.
Pretty mode (colored bar charts, `━━━` section headers, ASCII context-growth chart)
is never activated. The output is always the markdown pipe-table format:

```
# Session Analysis

## Summary

- **Format**: aggregate (277 sessions)
- **Tool calls**: 51346
...
| Tool | Calls | Errors | Success Rate |
|------|-------|--------|--------------|
| Bash | 25245 | 1851 | 93% |
```

What it should produce in pretty mode (per `write_pretty()` in
`crates/normalize-session-analysis/src/lib.rs` lines 908–1333):

```
━━━ Session Analysis ━━━

Format: aggregate (277 sessions)
Tool calls: 51346 (94.9% success)
...
━━━ Tool Usage ━━━
              Bash ██████████████████████████████ 25245 (1851 errors)
              Edit ████████████                   9945 (383 errors)
...
━━━ Context Growth ━━━
Turn  0: ▓▓░░░░░░░░░░░░░░░░░░  45.3K
Turn  5: ▓▓▓▓▓░░░░░░░░░░░░░░░  78.2K [!] High context
...
```

---

## Root cause

**File:** `crates/normalize/src/service/sessions.rs`
**Location:** `SessionsService::stats()` method, line 242 onward

The `stats` method is annotated with `#[cli(display_with = "display_output")]`.
`display_output` reads `self.pretty.get()` and dispatches to either `format_pretty()`
or `format_text()`. However, `stats` never sets `self.pretty`:

```rust
// display_output (line 54):
fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
    if self.pretty.get() {       // reads the Cell
        value.format_pretty()
    } else {
        value.format_text()
    }
}

// stats (line 242) — MISSING pretty/compact params and self.pretty.set():
pub fn stats(
    &self,
    grep: Option<String>,
    days: Option<u32>,
    ...
    // NO: pretty: bool
    // NO: compact: bool
) -> Result<SessionAnalysisReport, String> {
    // NO: self.pretty.set(resolve_pretty(..., pretty, compact));
    ...
    build_stats_data(...)
}
```

`self.pretty` is initialized to `false` in `SessionsService::new()` and never
updated in `stats`. So `display_output` always calls `format_text()`.

Compare with methods that work correctly (e.g. `list`, line 89):

```rust
pub fn list(
    &self,
    ...
    pretty: bool,           // declared
    compact: bool,          // declared
    ...
) -> Result<SessionListReport, String> {
    ...
    self.pretty.set(resolve_pretty(resolved_root, pretty, compact));  // set
    ...
}
```

The `SessionAnalysisReport::format_pretty()` implementation itself is complete and
correct — it lives in `crates/normalize-session-analysis/src/lib.rs` lines 901–1333
and is properly registered in the `OutputFormatter` impl at line 1343. The problem is
purely that `format_pretty()` is never called.

---

## How the bug became visible

Before commit `2351a201` (`polish(cli): fix --engine→--type in examples, add global
pretty/compact to config and sessions`), the `sessions` `#[cli]` attribute did not
declare `pretty`/`compact` as globals, so `--pretty` was not an accepted flag for
`sessions stats`. The bug was latent.

Commit `2351a201` added:
```rust
global = [
    pretty = "Human-friendly output with colors and formatting",
    compact = "Compact output without colors (overrides TTY detection)",
]
```

This makes `--pretty` appear in `sessions stats --help` and be accepted without error,
but does not cause the flag value to reach the method body. The flag is advertised but
silently ignored.

Additionally, without `pretty: bool, compact: bool` params, the TTY auto-detection
path in `resolve_pretty()` is also bypassed — so even running `sessions stats` in a
real terminal (which would normally auto-enable pretty mode) always shows text output.

---

## Is this a regression from the rank work?

No. The rank commits (72061e46, 9e124473, 2f8d06b9, 8a0f8d8d, 2f9c4545, d9e6e47e,
c3e07dbe) added `pretty_ranked_table`, `format_ranked_table`, `tier_color`, and
related helpers to `crates/normalize/src/output.rs`. None of these touched
`sessions.rs` or `normalize-session-analysis`. The shared `OutputFormatter` trait
default (`format_pretty()` falls back to `format_text()`) was not changed.

---

## Other commands with the same defect

Several other `SessionsService` methods use `#[cli(display_with = "display_output")]`
but are also missing `pretty: bool`/`compact: bool` params:

| Method | Has pretty params? | Has format_pretty()? |
|--------|-------------------|----------------------|
| `list` | yes | yes |
| `show` | yes | yes |
| `analyze` | yes | yes (dispatch via `display_analyze`) |
| `stats` | **NO** ← bug | yes (rich, colorized) |
| `ngrams` | **NO** | unclear (likely falls back) |
| `subagents` | **NO** | unclear |
| `plans` | **NO** | unclear |
| `mark` | **NO** | likely trivial |
| `unmark` | **NO** | likely trivial |

`stats` is the most impactful case because `SessionAnalysisReport::format_pretty()`
is a fully implemented, rich display that the user expects to see.

---

## Fix

Add `pretty: bool` and `compact: bool` to `stats()` and call `self.pretty.set()`:

```rust
pub fn stats(
    &self,
    grep: Option<String>,
    days: Option<u32>,
    since: Option<String>,
    until: Option<String>,
    project: Option<String>,
    all_projects: bool,
    format: Option<String>,
    limit: Option<usize>,
    group_by: Option<String>,
    root: Option<String>,
    mode: Option<SessionMode>,
    agent_type: Option<String>,
    sort: Option<String>,
    by_repo: bool,
    pretty: bool,    // ADD
    compact: bool,   // ADD
) -> Result<SessionAnalysisReport, String> {
    let limit = limit.unwrap_or(0);
    let root_path = root.as_deref().map(std::path::Path::new);
    let resolved_root = root_path.unwrap_or_else(|| std::path::Path::new("."));
    let project_path = project.as_deref().map(std::path::Path::new);
    let mode = mode.unwrap_or_default();
    self.pretty.set(super::resolve_pretty(resolved_root, pretty, compact));  // ADD
    ...
```

The same pattern should be applied to `ngrams`, `subagents`, and `patterns` (for
completeness), but `stats` is the critical fix.
