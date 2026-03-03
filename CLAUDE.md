# CLAUDE.md

Behavioral rules for Claude Code in this repository.

**References:** `docs/philosophy.md` (design tenets), `docs/architecture-decisions.md` (technical choices), `docs/session-modes.md` (working conventions).

## Publishing

**Published on [crates.io](https://crates.io/crates/normalize)** as 28 crates (+ 2 `publish = false`: `normalize-grammars`, `xtask`). All at v0.1.0 (early, in active development).

## Architecture

**Index-first:** Core data extraction (symbols, imports, calls) goes in the Rust index. When adding language support: first add extraction to the indexer, then expose via commands. All commands work without index (graceful degradation).

**CLI is generated from the service layer.** The CLI help and subcommands are NOT driven by `args.rs` / clap `#[derive(Subcommand)]` alone. The primary CLI registration happens in `service/analyze.rs` via `#[cli(...)]` proc-macro attributes on `AnalyzeService` methods. When adding a new analyze subcommand:
1. Create the analysis module (`commands/analyze/<name>.rs`) with report struct + `OutputFormatter`
2. Add a `display_<name>` method and a `pub fn <name>` method to `service/analyze.rs` using `#[cli(display_with = "display_<name>")]`
3. Add the module to `commands/analyze/mod.rs`
4. Add `assert_output_formatter::<Report>()` in `output.rs` test
5. Optionally add to `args.rs` `AnalyzeCommand` enum + dispatch in `mod.rs::run()` (for params-json/schema support)

Pattern for service methods:
```rust
fn display_foo(&self, r: &FooReport) -> String {
    if self.pretty.get() { r.format_pretty() } else { r.format_text() }
}

#[cli(display_with = "display_foo")]
pub fn foo(&self, root: Option<String>, limit: Option<usize>, pretty: bool, compact: bool) -> Result<FooReport, String> {
    let root_path = Self::root_path(root);
    self.resolve_format(pretty, compact, &root_path);
    Ok(analyze_foo(&root_path, limit.unwrap_or(30)))
}
```

**Output formats come from server-less for free.** Any report struct that derives `Serialize` + `schemars::JsonSchema` automatically gets `--json`, `--jsonl`, `--jq`, and `--schema` via the server-less proc macro. **You never implement JSON output.** The implementation work for a new command is `format_text()` and `format_pretty()` on the report struct — that's the `OutputFormatter` trait. Don't manually handle `--json` flags or write `serde_json::to_string` in command code.

**Balance agent vs tooling:** Both should progress in parallel. After significant agent work, pivot to tooling; after tooling sprint, check if agent could benefit.

**Language vs LocalDeps traits:** Two separate traits, two separate crates, no cross-dependency.
- `Language` (`normalize-languages`): Syntax/AST extraction — symbols, imports, exports, complexity. Implemented by ~98 languages. All methods are required (no defaults). Adding a language = implement this trait.
- `LocalDeps` (`normalize-local-deps`): Filesystem/package discovery — resolve imports, find installed packages, index external deps. Implemented by ~10 ecosystems. All methods have defaults (opt-in overrides). Adding package support = implement this trait.
- Assembly at top level: `deps_for_language(lang.name())` bridges syntax and deps lookups.
- When a trait grows beyond its domain, extract a new crate rather than expanding. Watch for: methods that only ~10% of impls override, methods that need filesystem access in a syntax trait, methods that need new dependencies.

## Core Rule

**Note things down immediately — no deferral:**
- Bugs/issues → fix or add to TODO.md
- Design decisions → docs/ or code comments
- Future work → TODO.md **right now, in the same response** — never say "I'll note this later"
- Key insights → this file
- Friction with normalize → TODO.md (we dogfood, friction = improvement opportunity)

"I'll add that to TODO.md" or "I'll note that" without immediately editing the file is the failure mode. Edit first, then respond.

**Keep docs in sync with code.** When renaming a command, adding a subcommand, or changing CLI structure: update `docs/cli/`, `README.md`, `LLMS.md`, `Architecture.md`, and `docs/cli-design.md` in the same commit. Stale docs compound — 200 commits of drift = a full day of cleanup.

**Triggers:** User corrects you, 2+ failed attempts, "aha" moment, framework quirk discovered → document before proceeding.

**Don't say these (edit first):** "Fair point", "Should have", "That should go in X" → edit the file BEFORE responding.

**Do the work properly.** When asked to analyze X, actually read X - don't synthesize from conversation. The cost of doing it right < redoing it.

**If citing CLAUDE.md after failing:** The file failed its purpose. Adjust it to actually prevent the failure.

**If the user corrects you at all, or you guessed at anything:** CLAUDE.md is probably missing something. Update it before proceeding.

**Language trait implementations must be honest about what the grammar provides.** Don't implement `container_kinds`, `container_body`, etc. based on what you *wish* the grammar modeled. If the tree-sitter grammar doesn't model a concept (e.g. markdown sections), return empty/None and handle it at a higher level — don't claim support you can't deliver. tree-sitter grammars are CSTs (concrete syntax trees), not ASTs — semantic structure (like "section = heading + content") must be derived, not assumed.

## From Session Analysis

Patterns from `docs/log-analysis.md` correction analysis:

- **Question scope early:** Before implementing, ask whether it belongs in this crate/module
- **Check consistency:** Look at how similar things are done elsewhere in the codebase before adding new patterns
- **Implement fully:** No silent arbitrary caps, incomplete pagination, or unexposed trait methods
- **Name for purpose:** Avoid names that describe one consumer ("tool registry" → "package index")
- **Verify before stating:** Don't assert AST node types, API behavior, or codebase facts without checking

## Dogfooding

**Use normalize, not builtin tools.** Avoid Read/Grep/Glob - they waste tokens.

```
./target/debug/normalize view [path[/symbol]] [--types-only]
./target/debug/normalize view path:start-end
./target/debug/normalize analyze [--complexity] [path]
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

**Always commit completed work.** The final step of any implementation task is `git commit`. After clippy + tests pass, commit immediately — don't wait to be asked. Uncommitted work is lost work.

## Session Handoff

Use plan mode as a handoff mechanism when:
- A task is fully complete (committed, pushed, docs updated)
- The session has drifted from its original purpose
- Context has accumulated enough that a fresh start would help

Before entering plan mode:
- Update TODO.md with any remaining work
- Update memory files with anything worth preserving across sessions

Then enter plan mode and write a plan file that either:
- Proposes the next task if it's clear: "next up: X — see TODO.md"
- Flags that direction is needed: "task complete / session drifted — see TODO.md"

ExitPlanMode hands control back to the user to approve, redirect, or stop.

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
- Hardcode file extensions anywhere — ever. Extension → language mapping belongs in the `Language` registry. Any `match ext { "rs" => ..., "ts" => ..., _ => default }` in non-registry code is wrong. Use `registry.language_for_extension(ext)` or equivalent.
- Ship mutating commands without `--dry-run` - every command that writes, deletes, or modifies anything must support `--dry-run` to preview what would happen
- Announce actions ("I will now...") - just do them
- Leave work uncommitted — after completing a task and tests pass, commit immediately without asking
- Create special cases - design to avoid them
- Create legacy APIs - one API, update all callers
- Add to the monolith - split by domain into sub-crates
- Do half measures - migrate ALL callers when adding abstraction
- Ask permission when philosophy is clear - just do it
- Return tuples - use structs with named fields
- Use trait defaults in `Language` - explicit impl required (but `LocalDeps` uses defaults by design)
- Put shared implementation helpers in the trait - helpers belong in a free function or a separate module, not as trait methods; trait methods are the interface, not the implementation
- Write stub implementations - every trait method must be fully implemented; `None`/empty is only correct when the concept genuinely doesn't exist in that language
- String-match AST properties - use tree-sitter structure
- Replace content when editing lists - extend, don't replace
- Cut corners with fallbacks - implement properly for each case
- Mark as done prematurely - note what remains
- Fear "over-modularization" - 100 lines is fine for a module
- Consider time constraints - we're NOT short on time; optimize for correctness
- Use path dependencies in Cargo.toml - causes clippy to stash changes across repos
- Use `--no-verify` - fix the issue or fix the hook
- Assume tools are missing - check if `nix develop` is available for the right environment

## Design Principles

**Unify, don't multiply.** One interface for multiple cases > separate interfaces. Plugin systems > hardcoded switches. When user says "WTF is X" - ask: naming issue or design issue?

**Simplicity over cleverness.** HashMap > inventory crate. OnceLock > lazy_static. Functions > traits until you need the trait. Use ecosystem tooling (tree-sitter queries) over hand-rolling.

**Rust/Lua boundary.** Rust for: native ops (tree-sitter, file I/O, subprocess), perf-critical code. Lua for: pure logic, user-facing scripting.

**Explicit over implicit.** Log when skipping. Location-based allowlists > hash-based. Show what's at stake before refusing.

**Separate niche from shared.** Don't bloat config.toml with feature-specific data. Use separate files for specialized data.

**Dynamic context > append-only.** Chatbot model (growing conversation log) is wrong for agents. Normalize uses context that can be reshaped, not just accumulated.

**When stuck (2+ attempts):** Step back. Am I solving the right problem? Check docs/philosophy.md before questioning design.

## Code Conventions

**OutputFormatter trait** (`crates/normalize/src/output.rs`):

All types that produce user-facing output should implement `OutputFormatter`:

```rust
impl OutputFormatter for YourType {
    fn format_text(&self) -> String {
        // Compact text (markdown, LLM-friendly, no colors)
        // Default format, used with --compact or no flags
        // Good for: piping, LLM consumption, copy/paste
    }

    fn format_pretty(&self) -> String {
        // Pretty text with colors and visualizations
        // Used with --pretty flag
        // Good for: terminal viewing, debugging
    }
}
```

Benefits:
- Consistent `--pretty`/`--compact`/`--json`/`--jq` across all commands
- No manual flag checking - use `OutputFormat::from_cli()` + `analysis.print(&format)`
- Respects `NO_COLOR` env var and TTY detection automatically
- `format_text()` is required, `format_pretty()` defaults to `format_text()` if not overridden

**When to use:**
- Analysis results (`SessionAnalysis`, complexity reports, etc.)
- Structured command output (stats, summaries, listings)
- **Not for:** Raw data dumps, interactive prompts, error messages
