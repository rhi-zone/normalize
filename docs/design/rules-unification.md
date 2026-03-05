# Rules Unification

Status: **partially implemented** (steps 1–6, 10 done; `rules run` now returns `DiagnosticsReport` with `--sarif`; 7–9 remaining)

## Problem

Three separate diagnostic/finding types exist across the codebase, plus ad-hoc structs for hardcoded checks:

### Structured diagnostic types

| Type | Crate | Fields | Severity | ABI |
|------|-------|--------|----------|-----|
| `Diagnostic` | `normalize-tools` | tool, rule_id, message, severity, location (required, precise), fix, help_url | Error/Warning/Info/Hint | Plain Rust |
| `Finding` | `normalize-syntax-rules` | rule_id, file, start/end line/col/byte, message, severity, matched_text, fix template, captures | Error/Warning/Info | Plain Rust |
| `Diagnostic` | `normalize-facts-rules-api` | rule_id, level, message, location (optional, coarse), related locations, suggestion | Hint/Warning/Error | `abi_stable` `#[repr(C)]` |

### Ad-hoc check output types

| Type | Module | Has file | Has line | Has message | Has severity | Has rule_id |
|------|--------|----------|----------|-------------|-------------|-------------|
| `BrokenRef` | check_refs | yes | yes | reference + context | no | no |
| `MissingExample` | check_examples | yes | yes | reference | no | no |
| `StaleDoc` | stale_docs | yes | no | no | no | no |
| `SecurityFinding` | report.rs | yes | yes | yes | Low/Med/High/Critical | yes |

### Rule engines

Two rule engines exist with separate CLI surfaces:

- `normalize syntax rules run` — runs tree-sitter pattern rules, outputs `Finding`
- `normalize facts rules` / `normalize facts check` — runs compiled/interpreted Datalog rules, outputs `facts-rules-api::Diagnostic`

Both are already managed together under `normalize syntax rules` (which has `--type syntax|fact|all`), but the name `syntax rules` is misleading since it also runs fact rules.

## Design: Unified Diagnostic

### Common core

Every issue/finding has:

```rust
pub struct Issue {
    pub file: String,
    pub line: Option<usize>,        // StaleDoc has no line
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
    pub rule_id: String,            // e.g. "broken-ref", "stale-doc", "circular-deps"
    pub message: String,
    pub severity: Severity,         // Error | Warning | Info | Hint
    pub source: String,             // "syntax-rules", "fact-rules", "check-refs", "clippy", etc.
    pub related: Vec<RelatedLocation>,
    pub suggestion: Option<String>,
}
```

Each existing type maps cleanly:
- `Finding` → fill all fields, `source = "syntax-rules"`
- `facts-rules-api::Diagnostic` → convert from `abi_stable` types, location may be None
- `normalize-tools::Diagnostic` → direct mapping, `source = tool`
- `BrokenRef` → `rule_id = "broken-ref"`, `severity = Warning`, message from reference
- `MissingExample` → `rule_id = "missing-example"`, `severity = Warning`
- `StaleDoc` → `rule_id = "stale-doc"`, `severity = Info`, `line = None`
- `SecurityFinding` → map severity (Low→Info, Med→Warning, High→Error, Critical→Error)

### Report struct

```rust
pub struct DiagnosticsReport {
    pub issues: Vec<Issue>,
    pub files_checked: usize,
    pub sources_run: Vec<String>,   // which engines/checks ran
}
```

`OutputFormatter::format_text` renders as:
```
file:line:col: severity [rule_id] message
  --> related_file:line
  suggestion: ...
```

This is the standard format already used by `normalize syntax rules run` and most linters.

### Where it lives

`normalize-diagnostics` crate (or fold into `normalize-output`). No dependency on `abi_stable` — conversion from `facts-rules-api::Diagnostic` happens at the boundary.

## Design: Top-Level `rules` Command

### Current state

```
normalize syntax rules run [--type syntax|fact|all] [--fix] [--sarif]
normalize syntax rules list [--type] [--tag] [--enabled]
normalize syntax rules enable/disable/show/tags/add/update/remove
normalize facts rules    # runs compiled dylib packs only
normalize facts check    # runs interpreted .dl files only
```

`syntax rules` already manages both engines. `facts rules` and `facts check` are redundant narrower entry points.

### Proposed

Lift `rules` to top level. Drop `syntax` prefix since it's not syntax-only.

```
normalize rules run [--engine syntax|fact|all] [--fix] [--sarif]
normalize rules list [--engine] [--tag] [--enabled]
normalize rules enable/disable/show/tags/add/update/remove
```

- Rename `--type` to `--engine` for clarity (syntax-rules vs fact-rules vs external-tools)
- Delete `normalize facts rules` and `normalize facts check` (subsumed)
- `normalize syntax` keeps `ast` and `query` (those are inspection, not rules)

### Long-term: hardcoded checks → rules

The analyze checks (`check-refs`, `stale-docs`, `check-examples`, `security`) are all "scan files, find violations." They could be:
1. Built-in rules with `rule_id`s, running through the same engine
2. Or at minimum, output through the same `DiagnosticsReport`

Phase 1: shared output format (all checks return `DiagnosticsReport`)
Phase 2: migrate to actual rules where the engine supports it

## Design: Rename `facts` → `structure`

`facts` is a Datalog term that means nothing to users. The subcommands are:
- `rebuild` — rebuild the code index
- `stats` — index statistics
- `files` — list indexed files
- `packages` — index external packages
- `rules` / `check` — run rules (moving to top-level `rules`)

After moving `rules`/`check` out, what's left is "build and query the structural index." `structure` captures this:

```
normalize structure rebuild
normalize structure stats
normalize structure files
normalize structure packages
```

Alternative: `index`. But `structure` is more descriptive of what's in it (symbols, imports, calls — the structural relationships in code).

## Design: Three Built-in Engines + User-Defined

### Built-in engines

| Engine | Rule format | What it sees | Examples |
|--------|------------|-------------|---------|
| `syntax` | `*.scm` (tree-sitter queries) | AST nodes in a single file | Pattern matching, naming conventions, banned syntax |
| `fact` | `*.dl` (Datalog) or compiled dylib | Extracted relations (symbols, imports, calls) | Circular deps, unused exports, layering violations |
| `native` | Rust code (not user-extensible) | Filesystem, index, git metadata, async I/O | check-refs (validates URLs), stale-docs (git mtime), check-examples, security |

`native` makes explicit what the hardcoded analyze checks already are — a third engine. These checks need capabilities that tree-sitter and Datalog can't express: HTTP requests, filesystem mtime comparison, glob expansion, full index queries.

### User-defined engines via dylib

`normalize-facts-rules-api` already has dylib plugin infrastructure (`RulePack` vtable over `abi_stable`). Generalize to a `CheckEngine` trait:

```rust
// In normalize-check-engine-api (or extend facts-rules-api)
#[sabi_trait]
pub trait CheckEngine {
    fn info(&self) -> EngineInfo;
    fn run(&self, context: &CheckContext) -> RVec<Diagnostic>;
}

pub struct CheckContext {
    pub root: RString,
    pub relations: Relations,          // extracted facts (symbols, imports, calls)
    pub file_list: RVec<RString>,      // indexed files
    // Future: filesystem handle, git handle, config
}
```

**Design choice: narrow vs wide context.**

The narrow approach (relations only, like today's `RulePack`) is safer and sandboxable — engines can't read arbitrary files or make network calls. The wide approach (filesystem + git + async) lets user engines do anything native checks do.

**Recommendation: start narrow, widen incrementally.** The `CheckContext` starts with just relations + file list (what `RulePack` already gets). Add filesystem read access as a second capability tier. Network/git access stays native-only until there's demand. Each tier is a separate trait method the engine can opt into:

```rust
// Tier 1: relations only (current RulePack capability)
fn run(&self, relations: &Relations) -> RVec<Diagnostic>;

// Tier 2: + file content access (read-only)
fn run_with_files(&self, relations: &Relations, files: &FileReader) -> RVec<Diagnostic> {
    self.run(relations)  // default: ignore files
}
```

This lets existing `RulePack` dylibs work unchanged while new engines can request more context.

### Unified `normalize rules run` across all engines

```
normalize rules run                        # all engines (syntax + fact + native)
normalize rules run --engine syntax        # tree-sitter rules only
normalize rules run --engine fact          # Datalog rules only
normalize rules run --engine native        # hardcoded checks only
normalize rules run --engine my-plugin     # user-provided dylib engine
```

All engines produce `Vec<Issue>` (or the ABI-stable equivalent). The CLI merges, sorts, and renders through `DiagnosticsReport`.

## Implementation Order

1. **Unified diagnostic types in `normalize-output`** — `Issue` + `DiagnosticsReport` + `OutputFormatter` (done)
2. **Add conversions** from `Finding`, `facts-rules-api::Diagnostic` (done — `diagnostic_convert.rs`)
3. **Migrate `check-refs`, `stale-docs`, `check-examples`** to return `DiagnosticsReport` (done)
4. **Unify into single `check` command** under `analyze` — `normalize analyze check [--refs] [--stale] [--examples]` (done)
5. **Lift `rules` to top level**, rename `--type` → `--engine`, delete redundant facts subcommands (done)
6. **Rename `facts` → `structure`** (done)
7. **Migrate `security`** to `DiagnosticsReport` (different severity mapping)
8. **Wire native checks as an engine** in `normalize rules run --engine native`
9. **Generalize `CheckEngine` trait** for user-defined dylib engines
10. **Revert `coverage`/`churn` enum wrappers** — no shared data shapes, split back to separate commands (done)
