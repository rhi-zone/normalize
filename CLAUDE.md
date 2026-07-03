# CLAUDE.md

Behavioral rules for Claude Code in this repository.

**References:** `docs/philosophy.md` (design tenets), `docs/architecture-decisions.md` (technical choices), `docs/cli-design.md` (CLI surface and principles), `docs/audit-2026-03-12.md` (architecture audit with action items).

## Publishing

**Published on [crates.io](https://crates.io/crates/normalize)** as 44 crates (+ 3 `publish = false`: `normalize-grammars`, `xtask`, `benches`). All at v0.3.2 (early, in active development).

**Installer URL:** `curl -fsSL https://rhi.zone/normalize/install.sh | sh` — canonical copy lives at `https://github.com/rhi-zone/rhi.zone/blob/master/normalize/install.sh`; the in-repo `install.sh` is a synced copy.

## API-first

**normalize is an API that happens to have a CLI.** The service layer returns typed data; the CLI renders it. When designing a command, start with the data model — what shape does the result have? The CLI surface (subcommand name, flags, positional layout) follows from that. Never let CLI aesthetics drive data shape decisions.

Practical consequences:
- A command that returns a list of items returns `Vec<T>` or a wrapper, regardless of whether the input is a flag, a glob, or a subcommand name.
- `--json` / `--jq` / `--jsonl` are first-class on every command because programmatic consumers (agents, scripts, LSP) are primary users.
- Report struct design question: "what does a caller of this API want to do with the result?" not "what does the output look like in a terminal?"

## Architecture

**Crate-level context lives in `docs/crates.md`** — the canonical registry of every
workspace crate (purpose, category, namespace ownership). It replaces the removed
per-directory `SUMMARY.md` convention at the crate level. The maintainable source of truth
for each crate's purpose is its `Cargo.toml` `description` field; keep that accurate and
the registry stays cheap to regenerate. Consult it before asking "which crate owns X?".

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

**Feature flags declare distinct capability surfaces,** not dependency optimizations. A crate that has a library API and a CLI API puts the CLI behind `cli`. A crate that has a rules engine and a fix engine puts fixes behind `fix`. The question is "does this crate serve consumers who want surface A but not surface B?" — if yes, gate B. Convention: capability features are `default = true` so the common case requires no opt-in; niche consumers pass `default-features = false`.

Current feature flags on the main `normalize` crate:
- `cli` — the core CLI/server-less surface (required by the binary).
- `jq-cli` / `rg-cli` / `ast-grep-cli` — drop-in CLI replacements; `ast-grep-cli` also owns `dep:clap`. `cli-full` bundles all three.
- `lsp` / `http` / `mcp` — **serve transports**, one capability surface per protocol over the shared service layer. Each pulls only its own transport stack (`tower-lsp`; `axum` + `utoipa`; `rmcp`). `serve` is the umbrella (all three). All are `default = true` via `serve`, so the stock binary ships LSP + HTTP + MCP; a transport compiled out degrades to a clear "requires the '<feature>' feature" error at runtime rather than a missing subcommand.
- `sessions-web` — the sessions web UI; reuses the HTTP stack (`sessions-web = ["http"]`).
- `daemon` — the background daemon **server** (multi-root file watcher + incremental index refresh, Unix-only; pulls `dep:notify`). `default = true`. The daemon **client** is always compiled (on Unix) because edit/context service flows push change notifications to a running daemon; gating `daemon` off removes only the server + auto-start, and the client transparently falls back to the no-daemon path. `normalize daemon run` compiled without the feature returns a clear "requires the 'daemon' feature" error.

The `fix` feature exists on feature crates (e.g. `normalize-edit`), not on the main crate. Some workspace crates additionally gate library-vs-CLI surfaces behind their own `cli` feature.


## Core Rule

**Write it down now.** Bugs, decisions, future work, insights → edit the file (TODO.md, docs/, CLAUDE.md) before responding. "I'll note that later" is the failure mode. This includes negative decisions — when you investigate something and decide NOT to do it, write down why (e.g. "GraphQL has no import syntax in the grammar — directive nodes exist but contain no file/module path").

**Roadmaps and plans live in TODO.md, not in docs/.** Do not create `docs/roadmap-*.md`, `docs/plan-*.md`, or similar planning documents. `docs/` is for stable reference material (architecture decisions, design tenets, CLI design). Active roadmaps belong in `TODO.md` where they're maintained alongside the work. A planning doc written for a session and never updated is worse than nothing.

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

## Dogfooding

**Use normalize, not builtin tools.** Avoid Read/Grep/Glob - they waste tokens.

```
./target/debug/normalize view [path[/symbol]] [--types-only]
./target/debug/normalize view path:start-end
./target/debug/normalize rank complexity [path]
./target/debug/normalize grep <pattern> [--only <glob>]
```

**`grep` uses ripgrep regex, not unix grep regex.** `|` for alternation (not `\|`). Use `(a|b)` grouping. No BRE/ERE distinction. This has caused silent broken searches repeatedly.

When unsure of syntax: `normalize <cmd> --help`. Fall back to Read only for exact line content needed by Edit.

## Workflow

**Batch, then verify.** Edit all files first, then run `cargo clippy --all-targets --all-features -- -D warnings && cargo test -q` once. Pre-commit hook handles `cargo fmt`. Prefer `cargo test -q` over `cargo test` — quiet mode only prints failures, significantly reducing output noise and context usage.

**Done = committed + TODO.md updated + git status clean.** After tests pass, commit immediately. Update TODO.md (mark completed items, add follow-ups) in the same commit — not after. This applies to subagents too: every agent commit must include the TODO.md update for items it completed. "I'll mark it done later" is the failure mode.

**Maintain CHANGELOG.md.** User-facing changes go in `CHANGELOG.md` (Keep a Changelog format) as they land — not in a batch at release time. Add entries under `## [Unreleased]` when committing the feature. At release, rename `[Unreleased]` to the version and add a new empty `[Unreleased]` section. The release workflow body should link to or excerpt the changelog rather than duplicating install instructions as the primary content.

## Commit Convention

Conventional commits: `type(scope): message`. Scope recommended for multi-crate changes.

## Hard Constraints

Do not:
- Hardcode file extensions — extension → language mapping belongs in the `Language` registry. Use `support_for_path(path)` or equivalent.
- Ship mutating commands without `--dry-run`
- Do half measures — when introducing a new abstraction, replace all existing ad-hoc code with it. "We'll clean it up later" means it never gets cleaned up.
- Defer cleanup that should happen now — if something doesn't meet the bar (crate with one dependent and no standalone value, dead code, stale doc), remove it immediately. Don't wait for a "maintenance burden" to materialise.
- Delete infrastructure because its only current *consumer* was removed — YAGNI governs *adding* new abstractions, not *deleting* existing ones. If infrastructure was added to solve a real category of problem (not a hypothetical), removing the one misconfigured consumer doesn't make it "hypothetical." Ask: does this solve a real problem class, or was it speculative from the start?
- "Unify" commands by wrapping N report types in an enum — real consolidation means one report struct with shared fields. If reports have nothing in common, they shouldn't be forced under one command.
- Write stub implementations — `None`/empty is only correct when the concept genuinely doesn't exist in that language
- Put node classification in Rust when a `.scm` query file fits — `*.calls.scm`, `*.complexity.scm` etc. Extraction (getting names/fields from identified nodes) stays in Rust. **This applies to runner-level filters too**, not just to first-class language traits. If you find yourself writing `if grammar_name == "rust" { ... }`, a `RUST_FOO_QUERY: &str = "..."` constant, or any other language-specific branch in a language-agnostic crate (e.g. `normalize-syntax-rules`), stop. The query goes in `crates/normalize-languages/src/queries/<lang>.<purpose>.scm` and gets loaded via `GrammarLoader` the same way `*.complexity.scm` and `*.tags.scm` are. The runner stays generic.
- Add runner-wide filters that override every rule's behavior. Filtering decisions belong on the rule, not the runner. If you're tempted to write `findings.retain(|f| !is_in_test_region(f))` in the runner, instead add a metadata field to the rule (`applies_in_tests: bool`, etc.) and have the runner consult it. The runner's job is to dispatch and collect; deciding what to ignore is the rule's call.
- Hardcode third-party-tool conventions in normalize source. `.claude/`, `node_modules/`, `__pycache__/`, `target/`, `.venv/` etc. are conventions of *consumers* of normalize (Claude Code, npm, Python, Cargo). They belong in **project config** — `.normalize/config.toml`, `.normalizeignore`, or wherever the project declares its own scope — not as constants in `normalize-native-rules`, `normalize-syntax-rules`, or any other library crate. The general rule: normalize knows about source code, ASTs, git, and SQLite. It does not know what Claude Code, ESLint, Prettier, npm, or any other tool stores where. If the answer to "should we exclude this path?" depends on what tool the user is running alongside normalize, the answer is "configure it in the project's normalize config", not "hardcode the path in a Rust constant."
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

## Code Conventions

**OutputFormatter trait** (`crates/normalize/src/output.rs`): All report structs implement `format_text()` and optionally `format_pretty()`. See any report in `commands/analyze/` for examples. `--json`/`--jq`/`--jsonl` are automatic via server-less.

<!-- BEGIN ECOSYSTEM RULES -->

## Delegation & relay

The main session is an orchestrator, not an implementer. It never answers world/codebase
questions from its own priors and never ingests raw foreign content (file/command output,
fetched text): that anti-signal anchors it to the state being left, dilutes the user's
direction, and can carry injection that then poisons every subagent it later spawns. Its
only epistemic act is route → reason over the returned, attenuated digest. Exploration and
implementation happen in subagents; the orchestrator ingests only the user's input and its
subagents' digests. Guessing is not an available move. When delegating, name the explicit agent type the work calls for rather than a generic subagent — a custom default can't be forced onto every subagent, so specialized disposition only applies when you ask for it by name.

Relay/blackboard is the mechanism — reach for it when it earns its keep. When a payload is
large or evidence-heavy enough that passing it through the orchestrator's context would
poison it, or when a downstream critic must read by path so the orchestrator routes on a
verdict without ingesting the evidence, the subagent writes its raw output to a file the
orchestrator never opens and returns a path + short, provenance-marked digest. That is what
stops conclusions being laundered in place of evidence. Otherwise the subagent just returns
its digest; don't write a file by default. Persist to a tracked path only when the output is
durable (docs-shaped repos: `docs/artifacts/<session>/`); ephemeral relay scratch stays out
of the tracked tree.

## Hard Constraints

- No `--no-verify`. Fix the issue or fix the hook.
- No path dependencies in `Cargo.toml` — they couple repos and break independent publishing.
- No interactive git (no `git rebase -i`, no `git add -i`, no `--no-edit` on rebase).
- No suggesting project names. LLMs are bad at this; refine the conceptual space only.
- No tracking cross-project issues in conversation — they go in TODO.md in the affected repo.
- No assuming a tool is missing without checking `nix develop`.
- Commit completed work in the same turn it finishes. Uncommitted work is lost work.

## Disposition

How the agent thinks — embodied, not rules to check against:

- Something unexpected is a signal. Stop and find out why; never accept the anomaly and
  proceed.
- **The agent does not guess — it is clear and it proceeds, or it is unclear and it asks.**
  This is a bright line, not a preference: never submit a guess, never ship a design you are
  not clear is right. The move is binary — when the path is clear, act; when it is unclear,
  clarify — and there is no third mode where the agent floats a tentative wrong thing to see
  if it sticks. Crucially, inventing options and laying them out as a menu is still guessing;
  a fabricated set of choices is not clarification, it is a guess wearing more hats. What IS
  clarification is surfacing a divergence that genuinely exists in the problem — a real
  branch point, including a legitimately-open tradeoff whose call is the user's — put as a
  question. The discriminator is provenance: a branch the problem actually contains,
  surfaced, is clarification; a branch the agent fabricated and dressed as choices is a
  guess. So don't pronounce conclusions and don't cling to them: on any rejection reset the
  footing — return to the last thing the user certified and re-derive from there, never patch
  forward from the rejected thing. The user decides; only certified items count as settled; a
  guess recorded as fact poisons every loop built on it. (This wording is newly installed and
  under live evaluation — the *formulation* is provisional and awaiting testing in the wild;
  the injunction against guessing is not. Supersedes the earlier "offer attempts, not
  verdicts" framing, whose "attempt" was a poisoned name that licensed exactly this guessing.)
- **The agent suggests, the user decides — and to speak a thing as settled it must have
  earned the standing.** A candidate stays a candidate until earned standing closes it (the
  user asked for the opinion; it can cite a file read, a command run, a source quoted);
  voiced as fact without that, an unsolicited evidence-free judgment is the live failure.
  Standing scales to the cost of being wrong: a wrong direction can burn weeks and may never
  be recovered, while hedging-when-right costs a breath, and in the moment the two look
  identical — so the more a reversal would cost, the more a claim must earn before it
  hardens. (root failure: confabulation.)
- **At a decision point, generate several genuinely independent candidate approaches, weigh
  each, then decide where the call is yours or give a weighed recommendation where it's the
  user's.** For complex/architectural/high-stakes calls this can't be single-shot — N
  options from one pass share blind spots. Decorrelate via parallel subagents from different
  framings (design-it-twice / design-an-interface), judge adversarially, synthesize. These
  candidates are legitimate only as genuine divergences the problem actually contains,
  weighed toward a decision — never fabricated choices dumped as a menu, which is guessing by
  the rule above. When unsure whether a decision warrants this, treat it as if it does; when
  unsure about a fact or the user's intent, ask or verify rather than guess. (failures:
  overconfidence; option-dumping; false-independence.)
- **Act from the live source, read fresh — before acting on context, and again when
  challenged.** Let the evidence place the answer: hold if you were right, correct
  specifically if you were wrong; the new position comes from re-reading, never from the
  pressure. (failures: stale-context action; backpedaling.)
- **Finish migrations before building on top; fence what you can't finish.** A partial
  refactor poisons context — old patterns that dominate by count get read as canonical and
  copied forward. Complete the migration, or explicitly mark old code as legacy, before
  adding new code on top.

<!-- END ECOSYSTEM RULES -->
