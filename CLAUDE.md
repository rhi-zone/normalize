# CLAUDE.md

Behavioral rules for Claude Code in this repository.

**References:** `docs/philosophy.md` (design tenets), `docs/architecture-decisions.md` (technical choices), `docs/cli-design.md` (CLI surface and principles), `docs/audit-2026-03-12.md` (architecture audit with action items).

## Publishing

**Published on [crates.io](https://crates.io/crates/normalize)** as 38 crates (+ 2 `publish = false`: `normalize-grammars`, `xtask`). All at v0.1.0 (early, in active development).

## API-first

**normalize is an API that happens to have a CLI.** The service layer returns typed data; the CLI renders it. When designing a command, start with the data model — what shape does the result have? The CLI surface (subcommand name, flags, positional layout) follows from that. Never let CLI aesthetics drive data shape decisions.

Practical consequences:
- A command that returns a list of items returns `Vec<T>` or a wrapper, regardless of whether the input is a flag, a glob, or a subcommand name.
- `--json` / `--jq` / `--jsonl` are first-class on every command because programmatic consumers (agents, scripts, LSP) are primary users.
- Report struct design question: "what does a caller of this API want to do with the result?" not "what does the output look like in a terminal?"

## Architecture

**Index-first:** Core data extraction (symbols, imports, calls) goes in the Rust index. When adding language support: first add extraction to the indexer, then expose via commands. Single-file commands (view, complexity, parsing) work without the index; cross-file features (import resolution, call graphs, dead code) require it and prompt the user to run `normalize structure rebuild`.

**CLI is generated from the service layer.** Subcommands come from `#[cli(...)]` proc-macro attributes on service methods, not `args.rs`. When adding a new subcommand:
0. **Check if it already exists under a different service.** Run `normalize --help` and check each service's subcommands. Commands have been moved between services before (e.g. `analyze ast` → `syntax ast` → duplicate `analyze parse` created because no one checked `syntax`).
1. **Decide where it lives.** If the subcommand belongs to an existing feature crate, add it there. If it's a new standalone feature, create a new crate with its own service. Only add to `commands/` in the main crate if it has no standalone value and no home elsewhere.
2. Look at an existing command for the pattern: `normalize view crates/normalize/src/service/analyze.rs` and pick a similar method as template.
3. Create the report struct + `OutputFormatter` in the owning crate (or `commands/<name>.rs` if staying in the main crate).
4. Add `assert_output_formatter::<Report>()` in `output.rs` test

**server-less is our own project** (dogfooding). Source at `/home/me/git/rhizone/server-less`. When the proc macro causes confusing behavior, investigate and fix it in server-less — don't document workarounds here. If a rule about server-less needs to exist in CLAUDE.md, that's a server-less UX bug.

**Generally useful functionality belongs in its own crate, not `normalize`.** The main crate is for CLI wiring (service layer, command dispatch, output formatting). The `normalize` binary is a consumer of the ecosystem, not a home for reusable logic.

**A crate should only exist if:** (a) it has multiple actual dependents within the workspace, or (b) it is clearly useful standalone — meaning it could be published independently and people would use it without normalize (e.g. `normalize-graph`, `normalize-code-similarity`). "Could theoretically be reused someday" doesn't count. If neither condition is met, the code belongs in `commands/` or the single crate that uses it.

The test for extraction: is this domain logic (algorithms, data models, extraction) or CLI wiring (formatting, dispatch, service layer)? Domain logic can be extracted when the above conditions are met. CLI wiring for a feature lives in the crate that owns that feature — a crate that owns a subcommand includes its own `#[cli]` service, report structs, and `OutputFormatter` impls. The main `normalize` crate just mounts them. Only cross-cutting wiring (command dispatch, global flags, output backend) lives in `normalize` itself. If it's purely "compute something and format it for this one command" with no standalone value, it stays in `commands/`.

**Feature flags declare distinct capability surfaces,** not dependency optimizations. A crate that has a library API and a CLI API puts the CLI behind `cli`. A crate that has a rules engine and a fix engine puts fixes behind `fix`. The question is "does this crate serve consumers who want surface A but not surface B?" — if yes, gate B. Convention: capability features are `default = true` so the common case requires no opt-in; niche consumers pass `default-features = false`. Current feature flags: `cli` (CLI/server-less surface), `fix` (autofix/`PlannedEdit` surface).


## Core Rule

**Write it down now.** Bugs, decisions, future work, insights → edit the file (TODO.md, docs/, CLAUDE.md) before responding. "I'll note that later" is the failure mode. This includes negative decisions — when you investigate something and decide NOT to do it, write down why (e.g. "GraphQL has no import syntax in the grammar — directive nodes exist but contain no file/module path").

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

Every directory with files should have a `SUMMARY.md` describing its purpose and contents. The pre-commit hook enforces this at `severity=error` via `normalize rules run --type native` (stale-summary rule).

**When making changes:**
- Keep `SUMMARY.md` up to date as a concise summary document — it describes what's there now, not what changed.
- Update it in the current directory if you add, remove, or significantly change files there.
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

**Batch, then verify.** Edit all files first, then run `cargo clippy --all-targets --all-features -- -D warnings && cargo test -q` once. Pre-commit hook handles `cargo fmt`. Prefer `cargo test -q` over `cargo test` — quiet mode only prints failures, significantly reducing output noise and context usage.

**Done = committed + TODO.md updated + git status clean.** After tests pass, commit immediately. Update TODO.md (mark completed items, add follow-ups) in the same commit — not after. This applies to subagents too: every agent commit must include the TODO.md update for items it completed. "I'll mark it done later" is the failure mode.

**Maintain CHANGELOG.md.** User-facing changes go in `CHANGELOG.md` (Keep a Changelog format) as they land — not in a batch at release time. Add entries under `## [Unreleased]` when committing the feature. At release, rename `[Unreleased]` to the version and add a new empty `[Unreleased]` section. The release workflow body should link to or excerpt the changelog rather than duplicating install instructions as the primary content.

## Context Management

**Use subagents to protect the main context window.** When a task requires broad
exploration (many files, deep search, multi-step research), delegate to an Explore or
general-purpose subagent rather than running the searches inline. The subagent returns
a distilled summary; raw tool output stays out of the main context.

**"Do it inline" poisons context.** Even a "small" edit that touches the service layer,
snapshot tests, and help output is the same scope as any other agent task. The temptation
to do it inline is always wrong — inline work accumulates tool output in the main context
and crowds out the actual conversation. If it needs a test run, it needs an agent.

Rules of thumb:
- Expect to search >5 files or run >3 rounds of grep/read → use a subagent
- Codebase-wide analysis (architecture, patterns, cross-crate survey) → always subagent
- Any edit that requires a `cargo test` or `cargo clippy` run → use an agent
- Single targeted lookup (one file, one symbol) → inline is fine

## Commit Convention

Conventional commits: `type(scope): message`. Scope recommended for multi-crate changes.

## Negative Constraints

Do not:
- Use Claude Code's auto-memory system (`~/.claude/projects/.*./memory/`) — it is unversioned, invisible to the user, and can't be diffed or backed up. Write behavioral changes directly to CLAUDE.md instead
- Hardcode file extensions — extension → language mapping belongs in the `Language` registry. Use `support_for_path(path)` or equivalent.
- Ship mutating commands without `--dry-run`
- Do half measures — when introducing a new abstraction, replace all existing ad-hoc code with it. "We'll clean it up later" means it never gets cleaned up.
- Defer cleanup that should happen now — if something doesn't meet the bar (crate with one dependent and no standalone value, dead code, stale doc), remove it immediately. Don't wait for a "maintenance burden" to materialise.
- Delete infrastructure because its only current *consumer* was removed — YAGNI governs *adding* new abstractions, not *deleting* existing ones. If infrastructure was added to solve a real category of problem (not a hypothetical), removing the one misconfigured consumer doesn't make it "hypothetical." Ask: does this solve a real problem class, or was it speculative from the start?
- "Unify" commands by wrapping N report types in an enum — real consolidation means one report struct with shared fields. If reports have nothing in common, they shouldn't be forced under one command.
- Write stub implementations — `None`/empty is only correct when the concept genuinely doesn't exist in that language
- Put node classification in Rust when a `.scm` query file fits — `*.calls.scm`, `*.complexity.scm` etc. Extraction (getting names/fields from identified nodes) stays in Rust.
- Use path dependencies in Cargo.toml — causes clippy to stash changes across repos
- Use `--no-verify` — fix the issue or fix the hook
- Read mutable globals (env vars, `lazy_static`, `OnceLock` of writable state) at call sites
  for things that should be construction-time config. Pass dependencies in. A `Client::new()`
  that pulls a socket path from `std::env::var(...)` on every invocation looks fine until
  two threads do it with different values, or a long-lived process (LSP, IDE plugin, library
  embedding) needs to talk to two daemons concurrently. Pattern: capture the env var **once**
  in a default-resolver, expose a `Client::with_X(x)` constructor that takes the resolved
  value, and have `Client::new()` delegate to it. Tests then construct with explicit values
  — no `serial_test`, no env-var serialization, no race. The general rule: configuration
  flows in via constructors, not out via globals at call sites.
- Shell out to external tools when a crate exists — use `fast_rsync` not `rsync`, `git2` not `git`,
  `zip` not `unzip`, etc. Shelling out adds a runtime dependency, breaks on systems where the tool
  is absent or has a different version, and loses structured error handling. Exceptions: tools that
  are genuinely part of the user's workflow and whose absence should be surfaced (e.g. a user-configured
  linter), or where the crate equivalent doesn't exist.

## LLM-Driven Workflows

**Text output is the agent interface.** LLMs consume the same `format_text()` output
as humans — not JSON. `--json` exists for programmatic/scripted consumers, not for
agents. JSON in an LLM context window is noise.

**`normalize init --setup` works for both humans and LLMs.** In a TTY it prompts
interactively; driven by an agent it reads the text output and issues commands
(`rules enable <id>`, `rules disable <id>`, etc.). No special mode needed — the same
interface serves both.

**Non-interactive ≠ non-functional.** Every command must work without a TTY. When
configuration is missing, print a clear actionable message to stderr and exit with a
non-zero code. Never silently return empty results.

## Design Principles

**Unify, don't multiply.** One interface for multiple cases > separate interfaces. Plugin systems > hardcoded switches. When user says "WTF is X" - ask: naming issue or design issue?

**Simplicity over cleverness.** HashMap > inventory crate. OnceLock > lazy_static. Functions > traits until you need the trait. Use ecosystem tooling (tree-sitter queries) over hand-rolling.

**Explicit over implicit.** Log when skipping. Location-based allowlists > hash-based. Show what's at stake before refusing.

**Separate niche from shared.** Don't bloat config.toml with feature-specific data. Use separate files for specialized data.

**When stuck (2+ attempts):** Step back. Am I solving the right problem? Check docs/philosophy.md before questioning design.

## Code Conventions

**OutputFormatter trait** (`crates/normalize/src/output.rs`): All report structs implement `format_text()` and optionally `format_pretty()`. See any report in `commands/analyze/` for examples. `--json`/`--jq`/`--jsonl` are automatic via server-less.
