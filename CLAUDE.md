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

**server-less is our own project** (dogfooding). The proc macro lives at `/home/me/git/rhizone/server-less`. When the proc macro causes confusing behavior, investigate and note it — don't work around silently. The `#[cli]` proc macro contract: **every `&self` method inside the annotated impl block = a subcommand.** This is by design, not a quirk. Helper methods (`display_*`, formatting, etc.) belong in a **separate** `impl` block above the `#[cli]` block. `display_with = "method_name"` references methods across impl blocks on the same struct. **Known bug:** `#[cli(name = "...")]` on nested services is ignored — the CLI subcommand name comes from the field name in the parent struct, not the `name` attribute.

**Generally useful functionality belongs in its own crate, not `normalize`.** The main crate is for CLI wiring (service layer, command dispatch, output formatting). The test: would anything other than one CLI command want this — another command, the LSP server, an external tool, a future library consumer? If yes, it belongs in a domain crate (`normalize-facts`, `normalize-session-analysis`, etc.). If it's purely "compute something and format it for this one command", it can stay in `commands/`. The `normalize` binary is a consumer of the ecosystem, not a home for reusable logic.


## Core Rule

**Note things down immediately — no deferral:**
- Bugs/issues → fix or add to TODO.md
- Design decisions → docs/ or code comments
- Future work → TODO.md **right now, in the same response** — never say "I'll note this later"
- Key insights → this file
- Friction with normalize → TODO.md (we dogfood, friction = improvement opportunity)

"I'll add that to TODO.md" or "I'll note that" without immediately editing the file is the failure mode. Edit first, then respond.

**Keep docs in sync with code.** When renaming a command, adding a subcommand, or changing CLI structure: update `docs/cli/`, `README.md`, `LLMS.md`, and `docs/cli-design.md` in the same commit. Stale docs compound — 200 commits of drift = a full day of cleanup.

**Triggers:** User corrects you, 2+ failed attempts, "aha" moment, framework quirk discovered → document before proceeding.

**Don't say these (edit first):** "Fair point", "Should have", "That should go in X" → edit the file BEFORE responding.

**Do the work properly.** When asked to analyze X, actually read X - don't synthesize from conversation. The cost of doing it right < redoing it.

**If citing CLAUDE.md after failing:** The file failed its purpose. Adjust it to actually prevent the failure.

**If the user corrects you at all, or you guessed at anything:** CLAUDE.md is probably missing something. Update it before proceeding.

**Language trait implementations must be honest about what the grammar provides.** Don't implement `container_body`, `refine_kind`, etc. based on what you *wish* the grammar modeled. If the tree-sitter grammar doesn't model a concept (e.g. markdown sections), return empty/None and handle it at a higher level — don't claim support you can't deliver. tree-sitter grammars are CSTs (concrete syntax trees), not ASTs — semantic structure (like "section = heading + content") must be derived, not assumed.

## From Session Analysis

Patterns from `docs/log-analysis.md` correction analysis:

- **Question scope early:** Before implementing, ask whether it belongs in this crate/module
- **Check consistency:** Look at how similar things are done elsewhere in the codebase before adding new patterns
- **Implement fully:** No silent arbitrary caps, incomplete pagination, or unexposed trait methods
- **Name for purpose:** Avoid names that describe one consumer ("tool registry" → "package index")
- **Verify before stating:** Don't assert AST node types, API behavior, or codebase facts without checking

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

**Batch cargo commands** to minimize round-trips:
```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo test
```
After editing multiple files, run the full check once — not after each edit. Formatting is handled automatically by the pre-commit hook (`cargo fmt`).

**When making the same change across multiple crates**, edit all files first, then build once.

**Minimize file churn.** When editing a file, read it once, plan all changes, and apply them in one pass. Avoid read-edit-build-fail-read-fix cycles by thinking through the complete change before starting.

**Always commit completed work.** The final step of any implementation task is `git commit`. After clippy + tests pass, commit immediately — don't wait to be asked. Uncommitted work is lost work. **The repo must be clean (`git status` = nothing to commit) when you finish a task.**

**Always update TODO.md at task end.** Mark completed items as done (use ~~strikethrough~~ or append `— DONE`), add newly discovered follow-up items, remove stale entries. A task is not complete until TODO.md reflects the new state. **Do this before the commit, not after** — the commit should include the updated TODO.md.

**These two steps are non-negotiable end conditions.** "I finished the implementation" is not done. Done = committed + TODO.md updated + `git status` clean. If you skip either step, you have left the repo in a worse state than you found it.

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

Use conventional commits: `type(scope): message`

Types:
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code change that neither fixes a bug nor adds a feature
- `docs` - Documentation only
- `chore` - Maintenance (deps, CI, etc.)
- `test` - Adding or updating tests

Scope is optional but recommended for multi-crate repos.

## Negative Constraints

Do not:
- Hardcode file extensions — extension → language mapping belongs in the `Language` registry. Use `support_for_path(path)` or equivalent.
- Ship mutating commands without `--dry-run`
- Leave work uncommitted or TODO.md stale — both must be updated in the final commit
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
