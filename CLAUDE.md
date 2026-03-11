# CLAUDE.md

Behavioral rules for Claude Code in this repository.

**References:** `docs/philosophy.md` (design tenets), `docs/architecture-decisions.md` (technical choices), `docs/cli-design.md` (CLI surface and principles), `docs/audit-2026-03-12.md` (architecture audit with action items).

## Publishing

**Published on [crates.io](https://crates.io/crates/normalize)** as 38 crates (+ 2 `publish = false`: `normalize-grammars`, `xtask`). All at v0.1.0 (early, in active development).

## Architecture

**Index-first:** Core data extraction (symbols, imports, calls) goes in the Rust index. When adding language support: first add extraction to the indexer, then expose via commands. Single-file commands (view, complexity, parsing) work without the index; cross-file features (import resolution, call graphs, dead code) require it and prompt the user to run `normalize structure rebuild`.

**CLI is generated from the service layer.** Subcommands come from `#[cli(...)]` proc-macro attributes on service methods, not `args.rs`. When adding a new subcommand:
0. **Check if it already exists under a different service.** Run `normalize --help` and check each service's subcommands. Commands have been moved between services before (e.g. `analyze ast` → `syntax ast` → duplicate `analyze parse` created because no one checked `syntax`).
1. Look at an existing command for the pattern: `normalize view crates/normalize/src/service/analyze.rs` and pick a similar method as template.
2. Create the analysis module (`commands/analyze/<name>.rs`) with report struct + `OutputFormatter`
3. Add `assert_output_formatter::<Report>()` in `output.rs` test

**server-less is our own project** (dogfooding). Source at `/home/me/git/rhizone/server-less`. When the proc macro causes confusing behavior, investigate and fix it in server-less — don't document workarounds here. If a rule about server-less needs to exist in CLAUDE.md, that's a server-less UX bug.

**Generally useful functionality belongs in its own crate, not `normalize`.** The main crate is for CLI wiring (service layer, command dispatch, output formatting). The test: would anything other than one CLI command want this — another command, the LSP server, an external tool, a future library consumer? If yes, it belongs in a domain crate (`normalize-facts`, `normalize-session-analysis`, etc.). If it's purely "compute something and format it for this one command", it can stay in `commands/`. The `normalize` binary is a consumer of the ecosystem, not a home for reusable logic.


## Core Rule

**Write it down now.** Bugs, decisions, future work, insights → edit the file (TODO.md, docs/, CLAUDE.md) before responding. "I'll note that later" is the failure mode.

**Keep docs in sync.** CLI changes → update `docs/cli/`, `README.md`, `LLMS.md`, `docs/cli-design.md` in the same commit.

**Verify before asserting.** Read the code before modifying it. Check how similar things work in the codebase before adding new patterns. Don't assert node types, API behavior, or codebase facts from memory — check the source.

**Fix root causes.** When corrected or when something fails: fix the underlying issue (docs, code, instructions) before proceeding. If a CLAUDE.md rule didn't prevent a mistake, the rule is broken — fix the rule.

**Be honest about capabilities.** Language trait implementations reflect what the tree-sitter grammar actually provides (CST, not AST). If the grammar doesn't model a concept, return empty/None — don't fabricate semantic structure.

## Language Quality

**Goal: maximum quality for every language we support.** Every supported language should have the best extraction we can provide — symbols, imports, calls, complexity, types — unless the language genuinely lacks a concept (e.g. Bash has no type system). "We haven't gotten to it yet" is a gap to close, not a state to accept.

**Grammars come from arborium or us.** We use arborium exclusively for curated grammars (we trust amos wenger's taste). For any language not in arborium's set, we write our own grammar — the Jinja2 grammar set this precedent. Don't pull in random tree-sitter grammars from the ecosystem.

**When investigating what a grammar supports**, use our own tools — don't read source code:
```
normalize syntax ast <file>           # see the full CST for a sample file
normalize syntax query <file> <query> # test a .scm query against a file
```
Write a small example file in the target language, parse it, and see what node types exist. This is faster and more reliable than reading grammar source code or guessing.

**When adding or improving a language:**
1. Add all applicable `.scm` query files (tags, imports, calls, complexity, types)
2. Implement the Language trait methods that the grammar supports
3. Don't leave gaps for "later" — if the grammar supports it, implement it now

## SUMMARY.md

Every directory with files should have a `SUMMARY.md` describing its purpose and contents. The pre-commit hook enforces this at `severity=error` via `normalize rules run --engine native` (stale-summary rule).

**When making changes:**
- Update `SUMMARY.md` in the current directory if you add, remove, or significantly change files there.
- Update ancestor `SUMMARY.md` files if the change affects a parent directory's description (e.g., adding a new crate, removing a module, changing a major interface).
- Rule of thumb: if a reader of the parent SUMMARY.md would be surprised by your change, update it.

**For context before making changes:**
- Read `SUMMARY.md` in the current working directory to understand the directory's purpose.
- Read ancestor `SUMMARY.md` files when working across multiple subdirectories or when you need broader architectural context.
- Example: before editing files in `crates/normalize-facts/src/`, read `crates/normalize-facts/src/SUMMARY.md` and `crates/normalize-facts/SUMMARY.md`.

**The pre-commit hook will block commits** if SUMMARY.md is stale (too many commits since last update) or missing and there are commits touching that directory. It also detects uncommitted content changes — if you staged file edits without updating SUMMARY.md, the check will catch it.

## Dogfooding

**Use normalize, not builtin tools.** Avoid Read/Grep/Glob - they waste tokens.

```
./target/debug/normalize view [path[/symbol]] [--types-only]
./target/debug/normalize view path:start-end
./target/debug/normalize analyze complexity [path]
./target/debug/normalize grep <pattern> [--only <glob>]
```

**`grep` uses ripgrep regex, not unix grep regex.** `|` for alternation (not `\|`). Use `(a|b)` grouping. No BRE/ERE distinction. This has caused silent broken searches repeatedly.

When unsure of syntax: `normalize <cmd> --help`. Fall back to Read only for exact line content needed by Edit.

## Workflow

**Batch, then verify.** Edit all files first, then run `cargo clippy --all-targets --all-features -- -D warnings && cargo test` once. Pre-commit hook handles `cargo fmt`.

**Done = committed + TODO.md updated + git status clean.** After tests pass, commit immediately. Update TODO.md (mark completed items, add follow-ups) in the same commit — not after.

## Session Handoff

Use plan mode as a handoff mechanism when:
- A task is fully complete (committed, pushed, docs updated)
- The session has drifted from its original purpose
- Context has accumulated enough that a fresh start would help

For handoffs (session complete or drifted): enter plan mode, write a short plan file
pointing at TODO.md, and ExitPlanMode. Do NOT investigate first — the session is already
context-heavy and about to be discarded. The fresh session investigates after approval.

For mid-session planning on a different topic: investigating inside plan mode is fine —
context isn't being thrown away.

Before the handoff plan, update TODO.md and memory files with anything worth preserving.

## Context Management

**Use subagents to protect the main context window.** When a task requires broad
exploration (many files, deep search, multi-step research), delegate to an Explore or
general-purpose subagent rather than running the searches inline. The subagent returns
a distilled summary; raw tool output stays out of the main context.

Rules of thumb:
- Expect to search >5 files or run >3 rounds of grep/read → use a subagent
- Codebase-wide analysis (architecture, patterns, cross-crate survey) → always subagent
- Single targeted lookup (one file, one symbol) → inline is fine

## Commit Convention

Conventional commits: `type(scope): message`. Scope recommended for multi-crate changes.

## Negative Constraints

Do not:
- Hardcode file extensions — extension → language mapping belongs in the `Language` registry. Use `support_for_path(path)` or equivalent.
- Ship mutating commands without `--dry-run`
- Do half measures — when introducing a new abstraction, replace all existing ad-hoc code with it
- "Unify" commands by wrapping N report types in an enum — real consolidation means one report struct with shared fields. If reports have nothing in common, they shouldn't be forced under one command.
- Write stub implementations — `None`/empty is only correct when the concept genuinely doesn't exist in that language
- Put node classification in Rust when a `.scm` query file fits — `*.calls.scm`, `*.complexity.scm` etc. Extraction (getting names/fields from identified nodes) stays in Rust.
- Use path dependencies in Cargo.toml — causes clippy to stash changes across repos
- Use `--no-verify` — fix the issue or fix the hook

## Design Principles

**Unify, don't multiply.** One interface for multiple cases > separate interfaces. Plugin systems > hardcoded switches. When user says "WTF is X" - ask: naming issue or design issue?

**Simplicity over cleverness.** HashMap > inventory crate. OnceLock > lazy_static. Functions > traits until you need the trait. Use ecosystem tooling (tree-sitter queries) over hand-rolling.

**Explicit over implicit.** Log when skipping. Location-based allowlists > hash-based. Show what's at stake before refusing.

**Separate niche from shared.** Don't bloat config.toml with feature-specific data. Use separate files for specialized data.

**When stuck (2+ attempts):** Step back. Am I solving the right problem? Check docs/philosophy.md before questioning design.

## Code Conventions

**OutputFormatter trait** (`crates/normalize/src/output.rs`): All report structs implement `format_text()` and optionally `format_pretty()`. See any report in `commands/analyze/` for examples. `--json`/`--jq`/`--jsonl` are automatic via server-less.
