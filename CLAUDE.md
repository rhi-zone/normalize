# CLAUDE.md

Behavioral rules for Claude Code in this repository.

**References:** `docs/philosophy.md` (design tenets), `docs/architecture-decisions.md` (technical choices), `docs/session-modes.md` (working conventions).

## Architecture

**Index-first:** Core data extraction (symbols, imports, calls) goes in the Rust index. When adding language support: first add extraction to the indexer, then expose via commands. All commands work without index (graceful degradation).

**Balance agent vs tooling:** Both should progress in parallel. After significant agent work, pivot to tooling; after tooling sprint, check if agent could benefit.

## Core Rule

**Note things down immediately:**
- Bugs/issues → fix or add to TODO.md
- Design decisions → docs/ or code comments
- Future work → TODO.md
- Key insights → this file
- Friction with moss → TODO.md (we dogfood, friction = improvement opportunity)

**Triggers:** User corrects you, 2+ failed attempts, "aha" moment, framework quirk discovered → document before proceeding.

**Don't say these (edit first):** "Fair point", "Should have", "That should go in X" → edit the file BEFORE responding.

## Dogfooding

**Use moss, not builtin tools.** Avoid Read/Grep/Glob - they waste tokens.

```
./target/debug/moss view [path[/symbol]] [--types-only]
./target/debug/moss view path:start-end
./target/debug/moss analyze [--complexity] [path]
./target/debug/moss text-search <pattern> [--only <glob>]
```

When unsure of syntax: `moss <cmd> --help`. Fall back to Read only for exact line content needed by Edit.

## Negative Constraints

Do not:
- Announce actions ("I will now...") - just do them
- Leave work uncommitted
- Create special cases - design to avoid them
- Create legacy APIs - one API, update all callers
- Add to the monolith - split by domain into sub-crates
- Do half measures - migrate ALL callers when adding abstraction
- Ask permission when philosophy is clear - just do it
- Return tuples - use structs with named fields
- Use trait default implementations - explicit impl required
- String-match AST properties - use tree-sitter structure
- Replace content when editing lists - extend, don't replace
- Cut corners with fallbacks - implement properly for each case
- Mark as done prematurely - note what remains

## Design Principles

**Unify, don't multiply.** One interface for multiple cases > separate interfaces. Plugin systems > hardcoded switches. When user says "WTF is X" - ask: naming issue or design issue?

**Simplicity over cleverness.** HashMap > inventory crate. OnceLock > lazy_static. Functions > traits until you need the trait. Use ecosystem tooling (tree-sitter queries) over hand-rolling.

**Rust/Lua boundary.** Rust for: native ops (tree-sitter, file I/O, subprocess), perf-critical code. Lua for: pure logic, user-facing scripting.

**Explicit over implicit.** Log when skipping. Location-based allowlists > hash-based. Show what's at stake before refusing.

**Separate niche from shared.** Don't bloat config.toml with feature-specific data. Use separate files for specialized data.

**When stuck (2+ attempts):** Step back. Am I solving the right problem? Check docs/philosophy.md before questioning design.
