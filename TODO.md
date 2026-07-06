# Normalize Roadmap

Last triaged: 2026-07-01

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

> *Open threads from a previous session. Treat as starting context, not instructions — verify relevance before acting.*

## Active open threads (advisory)

Three live threads from the 2026-06-29/07-01 session — verify state before acting:

- **CLI taxonomy inversion (B2–B12)**: B1 landed; B0's guide-regression + CLAUDE.md-crate-count parts landed, but **B0's server-less `#[cli(alias = "...")]` prerequisite is still PENDING** (blocks any verb-move batch's transitional aliases). The graph-crate blocker on B2/B3 is resolved (see below). Note: command-surface decomposition is the SAME move — see below. See [CLI command-taxonomy FULL INVERSION](#cli-command-taxonomy-full-inversion--seam-corrected-final-scope-high-priority) below.
- **Graph-crate split**: ✅ RESOLVED 2026-07-02 — refactored `normalize-graph` in place (pure algorithms split from presentation; deps `normalize-output`/`nu-ansi-term` dropped; characterization tests added). No standalone crate, no node-type genericization. Decision record (superseded resolution at top): `docs/artifacts/cli-taxonomy-2026-06-29/DECISION-graph-crate.md`. This unblocks B2 and B3. See [Graph crate refactor](#graph-crate-refactor-resolved-2026-07-02) below.
- **Main-crate decomposition audit**: ✅ DONE 2026-07-02 — full audit run; findings recorded in `docs/audit-2026-07-02.md`. Headline: the main crate is NOT a reservoir of extractable domain logic (reusable algorithms already in feature crates). Six small execution items + one rename remain (D1–D6 below), to be executed this session. See [Main-crate decomposition audit](#main-crate-decomposition-audit-done-2026-07-02) below.
- **Command-surface decomposition ≡ the B0–B12 CLI taxonomy inversion**: 🔄 IN PROGRESS 2026-07-03 — a second lens (CLAUDE.md's "crate owns its subcommand, main just mounts") reached the *same* move as the CLI taxonomy inversion, from the size-reduction direction. Main can shrink ~84k → ~30–34k (~21k forced-to-stay vendored CLIs → own core ≈ 9–13k). **The authoritative target taxonomy is already designed:** `docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md` (FINAL SCOPE, B0–B12). Audit (reconciled + open forks): `docs/audit-2026-07-03-command-surface-decomposition.md`. Sessions ✅ DONE 2026-07-03 (proof case, main src −8,086 LOC). Execution blocked on the open forks below (metrics A1/A2 remains; dataflow home RESOLVED 2026-07-03 → `normalize-facts`/`structure` verb, `normalize-cfg` ruled out by a `facts ⇄ cfg` compile cycle; `search` collision RESOLVED 2026-07-03 — drop `search`→`grep` alias at B7). See [Command-surface decomposition roadmap](#command-surface-decomposition-roadmap-in-progress-2026-07-03) below.

---

## CLI Taxonomy Migration

B0 (guide-regression test, CLAUDE.md crate count), B1 (`normalize-git` extraction), B2 (`graph` verb), and B3 (`architecture` verb) have landed. The graph-crate blocker on B2/B3 is **resolved** (2026-07-02, refactor-in-place — see below); B4–B12 can now proceed. The `#[cli(alias)]` server-less prereq is a separate server-less task; it does not block batches that don't move verbs yet.

## `normalize jq` fixed (2026-07-02)

- [x] **`normalize jq` was non-functional — every filter failed with `compile error: undefined
  Filter`.** Root cause was NOT a version skew (jaq-core 3.1.0 / jaq-std 3.0.1 / jaq-json 2.0.1 is
  the coherent latest release triple, already resolved and shared with the published
  server-less-core 0.6.0's `--jq` path). The vendored jq CLI (`crates/normalize/src/jq/filter.rs`,
  adapted from jaq v3.0.0-beta) never chained `jaq_core::defs()`/`jaq_core::funs()` — in jaq-core
  3.1.0 the core builtins were split out of std, so only std/json funcs were registered and every
  core filter was undefined. Added the core defs/funs chains (matching server-less-core). Same fix
  in `commands/sessions/analyze.rs::print_session_jq`. Bumped stale `*-beta` workspace reqs to the
  release triple. Verified `jq '.a'`, `length`, `map(.+1)`, `--jq` output mode, and `sessions
  analyze --jq` all work.

## Serve/daemon capability-surface feature pass

- [x] **Serve half (2026-07-02).** `normalize serve` transports are now gated capability
  surfaces: features `lsp` (`tower-lsp`), `http` (`axum` + `utoipa`), `mcp` (`rmcp`), umbrella
  `serve = [lsp, http, mcp]`, `default = [cli-full, serve]`. Made `axum`/`utoipa`/`tower-lsp`
  `optional` (rmcp already was). `serve/{http,lsp}` modules gated by their features; `mcp` keeps
  its always-present stub. `service/serve.rs` http/lsp bodies `#[cfg]`-branch to a clear runtime
  error when compiled out (mcp already did). Deleted the vestigial clap `ServeArgs`/`ServeProtocol`/
  `run` path in `serve/mod.rs` (unused — live dispatch is server-less `service/serve.rs`; this also
  fixed the latent `--features cli` clap-compile bug). MCP is now in the default build.
  `--no-default-features` drops axum 0.8/utoipa/tower-lsp/rmcp (verified via `cargo tree`; the
  residual axum 0.6 is an unrelated libsql→tonic transitive). CI `features` job added.
- [x] **Daemon half (2026-07-02).** Daemon **server** (watcher + incremental refresh) gated
  behind `daemon` feature (`default = [cli-full, serve, daemon]`); `notify` made `optional` and
  now leaves the dep tree when `daemon` is off (verified via `cargo tree -i notify`). The daemon
  **client** (`DaemonClient` + `Request`/`Response`/`Event` protocol + socket/lock helpers) stays
  always-compiled on Unix, so `service/edit.rs` and `service/context.rs` call sites are unchanged
  and fall through to their existing no-daemon path when no server is running/built. `maybe_start_daemon`
  is a no-op without the feature; `normalize daemon run` body `#[cfg]`-branches to a "requires the
  'daemon' feature" error (mirrors serve stubs). `Response::ok`/`err` and server-side tests gated
  too. CI `features` job gained a `cli,daemon` combo; the `cli`-only combo now also exercises the
  daemon-off path. **Capability-surface pass complete (serve + daemon).**
- [x] **Bare `--no-default-features` lib now compiles (2026-07-02).** The only stray references
  from non-cli-gated code into the cli-gated `crate::service` layer were the grammar auto-install
  paths: `commands/grammars.rs::ensure_grammars_first_use` and (at the time) `commands/init.rs::run_init`
  (the live `init` command is served from `service/mod.rs`). Fenced them — plus their now-core-dead
  helpers (`user_grammars_dir`/`dir_has_grammars`/`write_installed_stamp`/`INSTALLED_STAMP`,
  `generate.rs`'s `read_input`/`generate_cli_snapshot_code`) and imports — behind
  `#[cfg(feature = "cli")]`. No logic moved; the default and `cli` builds are byte-for-byte
  unchanged. The core lib now builds warning-free and stays slim (no notify/tower-lsp/axum-0.8/
  utoipa/rmcp; residual axum 0.6 is the unrelated libsql→tonic transitive). Also fixed the
  pre-existing `argv0` unused-variable warning under bare `cli` (only read inside the drop-in-CLI
  cfg blocks). CI `features` job gained a bare `--no-default-features` check so the core can't
  regress. **Capability-surface pass fully complete.** (Note: `run_init` was later removed entirely
  — see the `[walk]`-seeding fix below.)
- [x] **Duplicate `axum 0.6` / libsql remote stack resolved — now opt-in (2026-07-02).** The
  residual `axum 0.6.20` (and `tonic`/`tonic-web`/`libsql_replication`/`hyper 0.14`/`h2 0.3`/
  `tower-http 0.4`) reached the default build *only* via `libsql → tonic`. normalize uses only
  local libsql (`Builder::new_local`/`:memory:`), so the workspace `libsql` dep is now
  `default-features = false, features = ["core"]`, and the remote/replication stack is behind a
  new default-OFF `remote-sqld` capability feature on `normalize-facts`
  (`= ["libsql/replication", "libsql/remote", "libsql/sync", "libsql/tls"]`), re-exported as
  `normalize/remote-sqld`. Default build drops ~40 transitive crates; `cargo tree -i axum@0.6.20`
  and `-i tonic` now report "did not match any packages" on default features and return under
  `--features remote-sqld`. No code needed gating (no remote API was used). CI `features` job
  gained a `--features remote-sqld` check; asymmetry (active surfaces default-on vs. unused
  capability default-off) documented in `docs/architecture-decisions.md`. **This clears the last
  duplicate-dep finding from the audit.**
- [x] **`normalize init` `[walk] exclude` regression fixed + `run_init` removed (2026-07-02).** The
  `[walk] exclude` scratch-dir seeding lived only in `commands/init.rs::run_init`, which had been
  dead-except-tests since `c6a4b505` (2026-03-07); the seeding was added to that already-dead
  function later (`08fbc0dc`), so the shipped `service::init` never wrote a `[walk]` section. Moved
  the logic into the live served path: `service::init` now calls a new reusable
  `commands::init::build_walk_section(root)` that seeds the daemon baseline (`.git/`, `.normalize/`)
  plus auto-detected scratch dirs (`.claude/worktrees/` via `detect_scratch_dirs`), writes it as a
  discoverable section, and reports it in the `InitReport` change log (honored by `--dry-run`).
  Deleted `run_init` and the interactive `prompt_scratch_dirs` (unused by the non-interactive
  service path); re-homed the gitignore/TODO/scratch-dir/walk test coverage as direct helper tests
  (`#[cfg(test)]`, no cwd juggling). `SCRATCH_DIRS`/`detect_scratch_dirs`/`is_already_gitignored`/
  `build_walk_section` are now live via `service::init`. Docs (`docs/cli/init.md`,
  `architecture-decisions.md`, `commands/SUMMARY.md`) + CHANGELOG updated.

## Follow-ups (2026-06-29 branch consolidation)

- [x] **SUMMARY.md convention removed (2026-07-03).** The per-directory `SUMMARY.md`
  requirement, the `missing-summary`/`stale-summary` native rules, their config, and the
  CLAUDE.md section were removed — the convention was high-friction, chronically stale, and
  repeatedly blocked commits. This also retires the open `missing-summary` commit-count
  investigation (the rule no longer exists). Follow-up: `docs/crates.md` is the intended
  replacement for crate-level context; the `normalize view <dir>` SUMMARY.md-preamble
  feature is now inert (no files) and can be repurposed or removed separately.

- [x] **`docs/crates.md` crate registry landed (2026-07-03).** The intended crate-level
  replacement for SUMMARY.md now exists: every workspace crate with purpose, category, and
  CLI-namespace ownership (current vs planned-inversion), cross-checked against the
  inversion plan and the 2026-07-03 decomposition audit. Each crate's `Cargo.toml`
  `description` is the maintainable source of truth.
  - [ ] **Follow-up (future option, don't build now):** an `xtask` / `normalize` command
    that regenerates `docs/crates.md` from the workspace `Cargo.toml` `description` fields
    + the `service/mod.rs` mount table, so the registry can't drift. Manual for now.
  - [x] Fixed `normalize-grammars` `Cargo.toml` description (was placeholder "Normalize").

---

## CFG (Control Flow Graph) — Phase 1 ✓ + Phase 2 ✓ + Phase 3 ✓ + Phase 4 ✓

**Goal:** `normalize cfg <file> -f <function>` renders a Mermaid flowchart of a function's control flow.
**Phase 2 Goal:** def/use sites, SQLite persistence, Datalog facts, liveness analysis CLI.
**Phase 3 Goal:** Effects tracking — await, defer, yield, acquire/release, send/receive; `normalize analyze effects`.
**Phase 4 Goal:** Type-refined exception flow — `@cfg.exit.throw.type`/`@cfg.try.catch.type` captures; typed edges; `normalize analyze exceptions`.

**Commits 1–4 (scaffold, builder, mermaid, CLI): committed 2026-05-09**

- [x] Commit 1: Scaffold `normalize-cfg` crate with data model, empty builder, `GrammarLoader::get_cfg()`, SUMMARY.md
- [x] Commit 2: Structured-CFG builder + `rust.cfg.scm` query; snapshot tests
- [x] Commit 3: Mermaid renderer `Cfg::to_mermaid()` (included in commit 1)
- [x] Commit 4: `CfgService` + `normalize cfg` CLI; help snapshot; `docs/cli/cfg.md`
- [x] Commit 5: Python CFG query + fixtures
- [x] Commit 6: Go CFG query + fixtures
- [x] Commit 7: TypeScript CFG query (`typescript.cfg.scm`, `tsx.cfg.scm`) + 6 fixtures
- [x] Commit 8: JavaScript CFG query (`javascript.cfg.scm`) + 4 fixtures
- [x] Commit 9: Java CFG query (`java.cfg.scm`) + 5 fixtures (labeled break/continue validated)
- [x] Commit 10: Coverage matrix test (`coverage_matrix.rs`) classifying all languages as HAS_CFG / NOT_APPLICABLE / DEFERRED
- [x] Commit 11 (partial): Fixed pre-existing clippy error in all CFG test helpers (`is_some_and` collapse)
- [x] Commit 12: CFG Phase 1 batch — 69 additional `.cfg.scm` queries (C-family, JVM/functional, scripting, systems, domain/config); coverage matrix updated to 76 HAS_CFG; Lua + Jinja2 snapshot tests; dockerfile/query moved to NOT_APPLICABLE; asm/x86asm/uiua remain DEFERRED
- [x] Phase 2: `DefSite`/`UseSite` on `BasicBlock`; `@cfg.def`/`@cfg.use` captures (Rust/Python/Go); SQLite `cfg_blocks`/`cfg_edges`/`cfg_defs`/`cfg_uses` tables (schema v13); wired into `refresh_call_graph` and `reindex_files`; Datalog `cfg_block`/`cfg_edge`/`cfg_def`/`cfg_use` relations; `liveness.dl` builtin; `normalize analyze liveness <file> --function <name>` CLI command
- [x] Phase 3: `Effect`/`EffectKind` on `BasicBlock`; `BlockKind::Deferred/Acquire/Release`; `EdgeKind::Suspend/Resume`; `@cfg.effect.*` captures (Rust/Python/TS/JS/Go); SQLite `cfg_effects` table (schema v14); `CfgEffectFact` Datalog relation; `effects.dl` builtin; `normalize analyze effects <file> [--function <name>]` CLI command
- [x] Phase 4: `Edge.exception_type: Option<String>`; `@cfg.exit.throw.type`/`@cfg.try.catch.type` captures (Java, Python, JS/TS/TSX, C++, C#); `cfg_edges.exception_type` SQL column (schema v15); `cfg_edge` Datalog relation extended to 7 fields; `exception_flow.dl` builtin; Mermaid type labels; `normalize analyze exceptions <file> [--function <name>]` CLI command

**Remaining DEFERRED (Phase 1 cleanup):**
- asm, x86asm — assembly branches (jmp/je/jne) are at instruction level; need grammar inspection (not installed)
- uiua — array programming language, no standard query files; control flow is stack-based

**Follow-ups:**
- Java labeled break/continue: currently captured as `@cfg.exit.break`/`@cfg.exit.continue`; the label target is not resolved. Full label resolution (connecting break to the labeled outer loop rather than innermost) tracked here.
- Query validation: queries for non-installed grammars (most of batch A-E) are written against complexity-query node types; when grammars are installed, snapshot tests should be run to validate field names and node types are correct.
- Recursive CFG: nested control flow within arms/branches is currently a single Statement block; full recursion needs re-querying within each sub-range.
- Cyclomatic complexity from CFG (= edges - nodes + 2)
- LSP: expose CFG as an inlay hint or hover action
- Phase 2 follow-up: `@cfg.use` captures not yet written (only `@cfg.def` for Rust/Python/Go); add use captures to identify variables being read in each block
- Phase 2 follow-up: CFG data is not CA-cached — each `structure rebuild` re-builds CFGs for all files. Consider caching or making CFG rebuild optional.
- Phase 4 follow-up (Phase 5 territory): subtype hierarchy for exception type matching (e.g. `IOException extends Exception`). Phase 4 uses exact-match only; a throw of `IOException` won't match a `catch (Exception e)` unless Exception is the thrown type. Full subtype-aware matching needs type hierarchy facts.
- Phase 4 follow-up: add `@cfg.exit.throw.type` captures to more languages (currently Java, Python, JS/TS/TSX, C++, C#).

## Goal

Production-grade refactoring across all ~98 languages. Goal: rename, find-references,
extract, inline, move — correct, without LSPs, without false positives.

---

## 0.4 — working cross-language LSP with JetBrains-parity refactoring

**Concrete target**: a working LSP server exposing Find Usages, Rename, Safe
Delete, Extract Method/Variable, Inline, Change Signature across all ~98
supported languages — without LSP delegation, without false positives.
JetBrains-parity for refactoring is the bar; an LSP surface is how we
make it observable and usable from editors.

The 0.3.x line shipped the recipe scaffolding (rename, move, inline-variable,
inline-function, introduce-variable, add-parameter) but the harder recipes —
extract-function in particular — stalled because they need real semantic
infrastructure. 0.4 builds that infrastructure in five phases and surfaces it
via LSP. Phase design is in the sections below.

### Phase 0 — Cross-file name resolution (in progress)

**Scaffold committed (Commit 1):**
- [x] New Datalog predicates: `resolved_import`, `module`, `export`, `reexport`, `symbol_use`, `resolved_reference`, `resolved_call`, `module_search_path`
- [x] `ModuleResolver` trait in `normalize-languages::traits`
- [x] New crate `normalize-module-resolve`
- [x] `resolution.dl` Datalog rules

**Follow-up language resolvers (committed):**
- [x] Rust `RustModuleResolver` (Commit 2) — workspace_config/module_of_file/resolve for Cargo workspaces
- [x] TypeScript/TSX `TsModuleResolver` — relative imports, tsconfig.json paths/baseUrl, .js→.ts elision
- [x] JavaScript `JsModuleResolver` — relative imports, jsconfig.json paths, ESM/CJS
- [x] Python `PythonModuleResolver` — relative imports, src/ layout, `__init__.py` package detection
- [x] Go `GoModuleResolver` — go.mod module path, directory-based package resolution
- [x] Ruby `RubyModuleResolver` — require_relative, NotFound for bare require (gems)

**Pipeline and refactor integration (committed):**
- [x] Wire resolvers into `normalize structure rebuild` pipeline — `resolve_imports_via_module_resolver()` pass after `resolve_all_imports()` in full rebuild and incremental update
- [x] Tag `find_references` results with `confidence: "resolved" | "heuristic"` based on language resolver availability

**Extended language resolver coverage (committed):**
- [x] JVM languages: Java, Kotlin, Groovy, Scala (Maven/Gradle `src/main/<lang>` path conventions)
- [x] .NET languages: C#, VB, F# (namespace→file path mapping)
- [x] Swift (`SwiftModuleResolver` — SPM `Sources/<target>` directory targets)
- [x] Dart (`DartModuleResolver` — pubspec.yaml `package:` import resolution)
- [x] Zig (`ZigModuleResolver` — `@import` relative path resolution)
- [x] Elixir (`ElixirModuleResolver` — Mix `lib/` CamelCase↔snake_case)
- [x] Erlang (`ErlangModuleResolver` — 1:1 module=file)
- [x] Haskell (`HaskellModuleResolver` — Cabal `hs-source-dirs`)
- [x] OCaml (`OCamlModuleResolver` — capitalized stem convention)
- [x] Lua (`LuaModuleResolver` — `require` dot-path)
- [x] PHP (`PhpModuleResolver` — composer.json PSR-4 autoload)
- [x] Perl (`PerlModuleResolver` — `lib/` `::` path)
- [x] Clojure (`ClojureModuleResolver` — `src/` dot-namespace)
- [x] Common Lisp (`CommonLispModuleResolver` — workspace stem)
- [x] Scheme (`SchemeModuleResolver` — R7RS `.sld`/`.scm`)
- [x] Gleam (`GleamModuleResolver` — `gleam.toml` src/)
- [x] ReScript (`ReScriptModuleResolver` — bsconfig.json sources)
- [x] Language matrix test in `normalize-refactor/tests/cross_file.rs` — asserts resolver presence for all GP languages

**Phase 0 blockers — must be resolved before 0.4.0:**
- [ ] `normalize find-references --cross-file` command (depends on `structure rebuild`)
- [ ] Cross-file rename using resolved references (depends on confidence-tagged references)
- [ ] **C/C++/ObjC resolvers** — `#include` resolution requires `compile_commands.json` (compiler `-I` flags). Design needed: read `compile_commands.json` at workspace root; map each source file's include search paths; resolve `#include "foo.h"` against them. Blocking because C/C++ are among the most-used supported languages.
- [x] **Elm resolver** — `import Html.Attributes` → `Html/Attributes.elm` under source dirs from `elm.json`.
- [x] **D resolver** — `import mypackage.utils` → `mypackage/utils.d` under `source/` or `src/`. Reads `dub.json` `sourcePaths`.
- [x] **R resolver** — `source("./utils.R")` (relative file load) + `library(pkg)` (NotFound).
- [x] **Julia resolver** — `include("utils.jl")` (relative file include) + `using MyModule` (workspace package lookup via `Project.toml`).
- [x] **MATLAB resolver** — filename stem = function name; searches workspace root + `src/` + `lib/`.
- [x] **Prolog resolver** — relative `use_module('./utils')`, bare name search, `library(...)` → NotFound.
- [x] **Nix resolver** — `import ./utils.nix` relative path resolution; `<nixpkgs>` → NotFound.
- [ ] **Ada, Agda, Idris, Lean** — niche; design needs investigation. Add resolvers or explicitly document as NotApplicable with rationale. Not NotApplicable by default silence.

## 0.3.x post-release follow-ups (advisory)

Items that surfaced during the 0.3.1 release rodeo and may be worth a
second look — none are blocking, none are strictly committed:

- **Musl artifact end-to-end install never validated on a clean machine.**
  CI builds it cleanly and the wrapper script + bundled musl loader approach
  is principled, but no one has actually `tar xzf`'d the release on a fresh
  NixOS / Alpine / distroless container and verified the wrapper resolves
  correctly under `~/.local/bin/`-via-symlink, PATH lookups, etc. First user
  to install will be the integration test.

- **Crates.io rate-limit handling works but is slow.** publish.yml has
  Retry-After-aware retry; new-crate publishes still take 1-2 hours total
  for ~13 first-time-published crates due to the per-window cap. Could ask
  crates.io to raise our limit (their docs invite this for legitimate
  workspaces) — would shrink publish time to minutes.

- **Musl grammar build: switched to zig cc toolchain (libgcc_s issue resolved).**
  The zig toolchain uses its own static compiler-rt instead of emitting `-lgcc_s`,
  so the binary has no `libgcc_s.so.1` dependency. The CI and xtask grammar build
  now use `zig cc -target x86_64-linux-musl` consistently.
  NOTE: fully static musl binaries (crt-static=true) cannot dlopen — musl's static
  libc provides only a stub dlopen that returns ENOSYS. The release binary must
  remain dynamically linked to musl (crt-static=false) with the bundled loader
  wrapper to support grammar dlopen.

- **Some commits in this release line had to be retried 6+ times in CI**
  because each push surfaced a different latent bug (musl libm, libgcc_s,
  cargo fetch flag spelling, premature publish trigger). Pattern: each fix
  was correct in isolation but downstream effects only showed up on the
  next CI run. A `cargo build --target x86_64-unknown-linux-musl` smoke
  test in `ci.yml` (PR-time) would catch most musl-target issues before
  they hit release.yml. Currently `ci.yml` only builds the host target.

- **0.3.0 partial-publish leftovers on crates.io.** 10 crates were
  published at 0.3.0 before the failed run was caught. They live alongside
  their 0.3.1 versions; if anyone pinned to `=0.3.0` they'd get a
  potentially-broken build (some crates had path deps without `version =`
  pinned, which is what caused the failure). Could yank the 0.3.0 versions
  for safety.

- [x] **Toolchain drift: pinned via `rust-toolchain.toml` at `1.95.0`; clippy hard-gated in pre-commit; hook auto-installs via `nix develop` shellHook (2026-05-29).**
  `rust-toolchain.toml` pins `channel = "1.95.0"` with components and musl target matching `flake.nix`.
  The pre-commit clippy invocation is now a hard gate (was non-blocking with `|| { echo "non-critical" }`).
  The flake's `shellHook` diff-guards and auto-installs `.git/hooks/pre-commit` on every `nix develop` entry.
  **Residual:** `flake.nix` still uses `fenixPkgs.stable` rather than reading `rust-toolchain.toml` via
  `fromToolchainFile` (option declined — keep flake simple). If `nix flake update` advances fenix `stable`,
  the in-shell toolchain and `rust-toolchain.toml` channel will diverge. Keep them in sync manually when
  bumping the toolchain.

- [x] **Daemon flake `config_edit_triggers_reload_event` — RESOLVED (2026-07-02).**
  Root cause was a **real daemon bug**, not test timing (the earlier
  "test-interaction/inotify-saturation" diagnosis was wrong): the notify
  dispatch loop classified *any* event on `.normalize/config.toml` as a config
  change, including inotify `Access(Open)` **reads**. The daemon reads config.toml
  constantly during startup indexing/prime (`NormalizeConfig::load`), so its own
  reads fired a phantom config reload that emitted a spurious `IndexRefreshed
  { files: 0 }`; the test's subscriber consumed that as the config-edit signal
  and stopped before the real edit. Fix: dispatch loop now ignores `Access(_)`
  (read) events — real mutations arrive as `Create`/`Modify`/`Remove`. The test
  was also hardened against the broadcast-attach race (re-edits config until the
  subscriber, which attaches asynchronously, observes the reload). Proven: 40/40
  single-test + 25/25 full `daemon_push` suite green (was ~2/15 flaky).

- [x] **Daemon native-refresh backlog: unbounded channel → per-root coalescing**
  (2026-05-29). The daemon watched each repo's `.git/index` and pushed the root
  into an unbounded `mpsc::channel` on every change; the consumer drained ~0.9/s
  while heavy git churn enqueued ~5/s, building a ~57k-deep backlog that pegged
  ~2 cores for hours. Because each native refresh re-reads current disk state, N
  queued refreshes of the same root are redundant — coalescing to "latest wins"
  is exact, not lossy. Replaced the channel with a `DirtyRoots` set
  (`Mutex<(HashSet<PathBuf>, bool)>` + `Condvar`): producer marks a root dirty,
  consumer blocks then drains all distinct dirty roots and refreshes each once.
  Backlog is now bounded by the number of watched roots regardless of churn.
  Also: (a) added a second spin signal `record_native_refresh_and_detect_spin`
  for the `.git/index` path (the overlap-based detector only fired on
  `.normalize/`-overlapping changes, so a `.git/index`-driven backlog never
  tripped it) — flags when refresh density is high AND the coalescing queue is
  not draining; the dispatch loop now consults spin backoff for *both* the full
  and native refresh branches. (b) Fixed the SQLite write failures: the
  `daemon_diagnostics has no column named issues_blob` error is now self-healed
  at `FileIndex::open` (PRAGMA table_info check → drop+recreate on stale shape),
  and the `cannot start a transaction within a transaction` error is fixed by
  rollback-on-error + a defensive ROLLBACK-before-BEGIN (`begin_clean` /
  `commit_or_rollback`) at every `BEGIN…COMMIT` site in `normalize-facts/index.rs`
  (previously, an error mid-transaction returned early leaving the txn open on the
  reused connection, wedging every subsequent refresh). (c) Graceful `daemon stop`
  now removes the socket + lock files before `process::exit` (flock is released by
  the OS on exit regardless; startup already tolerates a stale lock file).

- [x] **Daemon spin observability + self-defense** (follow-up to commit
  365f9ee6, which fixed the acute walk-exclude cause). Two features landed:
  (1) auto-started daemons now log to `~/.config/normalize/daemon.log` instead
  of `/dev/null` (the reason both spins went unnoticed) — keyed off
  `NORMALIZE_DAEMON_LOG` set on the spawned child; foreground `daemon run`
  still logs to the terminal. (2) overlap-based spin detector: when a refresh's
  changed-set overlaps the root's own state dir (config-derived via
  `get_normalize_dir`) at ≥5 refreshes/10s, the daemon flags the root, backs
  off refreshes for 30s (per-root), WARNs, and records a `SpinWarning` surfaced
  by `daemon status`. Indexing is never silently dropped — backoff + loud warning
  only. Follow-up corner cut: log file is append-only with no rotation
  (`tracing-appender` not yet a dep); add daily rotation if the file grows
  unbounded in practice.

## Structured-metadata symbol search (0.4 design)

Replaces the embedding-based symbol search dropped in 0.3.0. The design sits
under the broader rhizone direction — arbitrary structured metadata as the
primary shape for facts about *anything* (symbols, files, sessions, rules,
manifests, etc.) — not a normalize-local tag system. "Tags" is a degenerate
case (flat key, optional string value); we want the full structured shape from
day one so we don't paint ourselves into a corner.

Each symbol gets a metadata document — nested, typed, schema-aware:
```
{
  kind: "function",
  module: "crates/foo/src/bar.rs",
  effect: { io: true, async: false },
  complexity: { cyclomatic: 12, cognitive: 8 },
  domain: ["auth", "session"],
  tested: { has_test: true, coverage_pct: 73 },
  ...
}
```

Sources of metadata, cheap → expensive:
1. Structural (free): kind, module path, complexity, sync/async, has-test
2. Query-derived (.scm captures producing structured fragments): "uses tokio::spawn" → `effect.async = true`
3. LLM-derived (cached by blake3(body)): domain classification, summary
4. User-supplied: attribute / annotation / sidecar — `#[normalize::meta(...)]`,
   `// @meta domain: auth`, `.normalize/meta/<symbol>.toml`

Storage: a structured doc per symbol (rkyv blob in SQLite, or columnar where
schema is fixed). Query: predicate evaluation over structure — path-into-doc
+ match — composes with existing structural primitives. The exact query
surface needs design (jsonpath-shaped? jq-shaped? typed predicates?) but it
has to be richer than tag-set intersection.

This is not normalize-specific. The same shape applies to:
- `.normalize/context/*.md` frontmatter (already structured YAML)
- session metadata across agent formats
- manifest data (`normalize-manifest`)
- rule metadata (already has nested fields)

Aligning normalize's symbol metadata shape with the cross-project direction is
part of the work — the schema lives somewhere shared, not buried in normalize.

BM25 over (name + leading-doc + path-tokens) via SQLite FTS5 covers cheap
lexical search alongside the structured query path. Embeddings could return
as a niche escape hatch, but not in the default path.

---

## P0 — Blocking / Broken / Incoherent

### ~~`syntax query` silently ignores top-level `[...]` alternation~~ — FIXED

Extended `is_sexp_pattern()` in `crates/normalize-syntax-rules/src/query.rs` to also
return `true` when the pattern starts with `[`. Unit test added in `query.rs`.
Empirically verified: `[(identifier) @i (line_comment) @c]` now returns matches; nested
alternation and ast-grep patterns unaffected.

### `normalize context context` duplicate subcommand — fixed in server-less, unblocked by 0.5.0

`normalize context --help` shows `context` as a named subcommand (duplicate of the service itself).
Root cause: server-less registered `#[cli(default)]` methods as BOTH the default action AND a
named subcommand. Fixed in server-less (commit f7bc30b): default methods are now suppressed from
the subcommand list entirely. server-less 0.5.0 is now published (2026-06-19) carrying this fix —
the bump to 0.5.0 (see "server-less 0.5.0 adoption" below) lands it in normalize.


## CLI Audit Backlog (2026-06-29)

Catalogue from `docs/artifacts/cli-audit-2026-06-29/00-triage.md`. T1 = hard-constraint
violations or actively broken; T2 = correctness/consistency; T3 = polish.

### Tier 1 — Actively broken / hard-constraint violations

- [x] **T1-3 — All `trend` metric commands broken (stale worktree `[embeddings]` config)** — FIXED.
  Downgraded `process::exit(1)` in `NormalizeConfig::load` to a recoverable warning; serde
  already ignores unknown sections. All four commands (`trend complexity`, `trend length`,
  `trend density`, `trend test-ratio`) now work.
- [x] **T1-1 — Index-dependent commands exit 0 + empty JSON on missing index** — FIXED. Centralized `index::require_import_graph` guard; `view graph/dependents/import-path`, `rank imports/depth-map/layering`, and `analyze architecture` now exit non-zero with an actionable message when the import graph is empty.
- [x] **T1-2 — 22 mutating commands ship without `--dry-run`** — FIXED. `edit redo`, `rules run --fix`, `kg write`, `rules add/update/remove/setup`, `ratchet`/`budget` CRUD, `sessions mark/unmark`, `structure rebuild` all have `--dry-run`. `daemon start/stop`, `update`, `generate client/cli-snapshot` judged out of scope (see CHANGELOG). `structure packages` still needs `--dry-run` (see T1-4).
- [ ] **T1-4 — `structure packages`** — output half fixed (no longer silently empty); still needs `--dry-run` (writes global grammar cache).
- [x] **T1-5 — `rules show stale-summary` lookup bug** — FIXED. `show_rule` now searches native-rules registry too.
- [x] **T1-6 — Broken guides + stale `kg --help` examples** — `guide analyze` references moved
  commands; `guide rules` references `analyze node-types` (now `syntax node-types`); `kg --help`
  epilogue references `kg create/link/query/show` (none exist).

## server-less 0.5.0 adoption

server-less 0.5.0 shipped 2026-06-19 with `--manual` (a whole-tree CLI reference surface)
motivated by normalize's ~150-command nested CLI. Adoption tasks:

- [x] **Bump server-less → 0.6 and publish** — Done. Workspace + three separately-pinned crates (`normalize-cfg`, `normalize-budget`, `normalize-ratchet`) require `0.6`. Adapted to the `ConfigTrait`→`ConfigLoad` rename. 0.6.0 published; `[patch.crates-io]` override removed (2026-07-01).
- [ ] **Adopt `--manual` for the nested command tree.** `normalize --manual` (and
  `normalize <subtree> --manual`) now emits the entire ~150-command surface as one greppable
  document — text by default, `--json`/`--jsonl`/`--jq` for structured. Closes the "no whole-tree
  docs dump" gap (server-less `docs/artifacts/normalize-cli-docs/cli-surface.md`).
- [ ] **Retire whole-tree-docs workarounds obsoleted by `--manual`.** Re-evaluate
  `normalize generate cli-snapshot` (recursively walks `--help` to enumerate the tree — `--manual`
  now does this natively) and complete the `--schema` pre-clap-hack retirement (cross-reference the
  existing "Retire `--schema` hack" entry — its blocking dependency, the manual surface, has now
  shipped).
- [ ] **Verify the reserved-flag collision guard after the bump (low risk).** 0.5.0 turns a
  `#[param]` name colliding with an injected global flag (`manual`, `json`, `jq`, `*-schema`, etc.)
  into a compile error. A grep found no colliding param names in normalize today, so a clean build
  after the bump should confirm — flagged for awareness, not a known breakage.

### CLI capability-wiring invariant (consumer-side adoption)

server-less audit (2026-06-28) generalized the `--pretty` footgun into a class:
`docs/design/cli-capability-wiring-invariant.md` in the server-less repo. server-less 0.6.0
(branch `feat/cli-capability-wiring-invariant`) shipped the `CliGlobals` sink mechanism;
normalize adopted it (2026-06-29) via a local `[patch.crates-io]` dogfood override.

- [x] **Adopt `CliGlobals` + fix the 8 BROKEN `--pretty` commands (2026-06-29).** Replaced the
  per-method `pretty`/`compact` params + `resolve_*` calls across 12 services with one
  `CliGlobals` sink per service that stashes the raw flags into `pretty_raw`/`compact_raw`
  cells; root-aware TTY/config resolution stays per-command via `resolve_format(root)`. This
  fixed the 8 strict-BROKEN commands (`sessions stats`/`subagents`, `analyze architecture`/
  `cross_repo_health`, `rank files`/`size`/`ceremony`/`contributors`) — `--pretty` now
  dispatches `format_pretty()`. Rules resolves lazily in its display bridges (root-independent).
  `normalize-ratchet`/`normalize-budget`/`package` had no real `format_pretty`, so their
  service-level `--pretty` advertisement (own `global = [...]` + dead `pretty` Cell) was
  removed entirely rather than wired to an inert sink. **Correction:** only the service-level
  flag was removed; the root-level `--pretty`/`--compact` globals (injected by server-less
  into every command) still appear in help and are accepted — they are silently inert on these
  commands and on any `analyze` method whose report has no real `format_pretty` (see advertised-no-op
  item below). See `docs/artifacts/sessions-stats-output-2026-06-20/pretty-wiring-audit.md`.
- [x] **Publish server-less 0.6.0 and drop the `[patch.crates-io]` override** — DONE (2026-07-01). 0.6.0 published to crates.io; patch block removed from workspace `Cargo.toml`.
- [ ] **Phase 2 — `display_with`→`render` (dead-dispatch class).** The 6 `edit` refactor
  commands + `syntax node-types` have real `format_pretty` impls reached by a `display_with`
  fn that calls `format_text()` unconditionally (3b in the design doc — opaque-body footgun,
  not the global-flag class). Server-less 0.6 did NOT subtract `display_with` (design-A
  deferred). Fix these by making their display fns dispatch on the pretty state, or adopt the
  upstream `render(mode)` if it lands.
- [ ] **Root-global `--pretty` advertised-no-op (framework-level).** The root `--pretty`/
  `--compact` globals (injected by server-less into every subcommand) are silently accepted
  but do nothing on commands whose report has no real `format_pretty`. Affected: the 7 `analyze`
  methods whose reports only have the default `format_pretty` (`security`, `docs`, `activity`,
  `repo_coupling`, `liveness`, `effects`, `exceptions`), plus `normalize-ratchet`/`normalize-budget`/
  `package`. Decision deferred: whether server-less should suppress root globals on commands
  whose report type has no real pretty (needs type-visibility at macro-expansion time, i.e.
  the same design-A/B tradeoff explored in the capability-wiring design doc). Known limitation
  for now; tracked here for Phase 2+ consideration.
- [x] **Audit normalize for `#[param(name)]` / `#[param(default)]` on `#[cli]` methods.**
  0.6 honors `#[param(name)]` (renames the flag). Audited 2026-06-29 (see CLI audit T2-3):
  exactly two annotations exist, both in `normalize-budget/src/service.rs` (`measure`, `add`),
  both `name = "diff-ref"` on field `base_ref` — so `budget measure`/`budget add` `--base-ref`
  (0.5.x) silently became `--diff-ref` (0.6.x). **Verdict: keep the rename** (the author opted
  in deliberately; 0.6 made the CLI match the help text). Follow-up: add a CHANGELOG `[Unreleased]`
  entry documenting the rename; optionally accept `--base-ref` as a hidden alias for one release.

### CLI audit 2026-06-29 backlog

Full catalogue: `docs/artifacts/cli-audit-2026-06-29/00-triage.md` (consolidates five audits).
Recording reality, not a commitment to do it all now. Items already tracked elsewhere are
cross-referenced, not duplicated.

**Tier 1 — hard-constraint violations + actively broken (do first):**
- [x] **T1-1: Index-dependent commands exit 0 + empty JSON on missing index** (HARD CONSTRAINT —
  "never silently return empty results"). DONE: centralized `index::require_import_graph` guard
  keys on the raw `imports` row count; `view graph/dependents/import-path`, `rank imports/
  depth-map/layering`, and `analyze architecture` now exit non-zero with an actionable
  `structure rebuild` message when the import graph is empty. A populated index with a
  genuinely-empty *query* (e.g. `import-path A B` with no path) still exits 0. Errors now emit
  `{"error": "<msg>"}` under `--json` (server-less generic error path; closes the JSON half of
  T2-2 too). NOTE: `rank call-complexity` was in the audit list but builds its call graph by
  parsing files directly (no sqlite index) — it has no missing-index state, so no guard was
  added (audit HIGH-6 was a misdiagnosis). FOLLOW-UP: a discrete `"requires_index": true` JSON
  field (vs. the generic `{"error":…}`) would need typed errors through server-less; the message
  names the rebuild step, which is sufficient for now.
- [x] **T1-2: 22 mutating commands ship without `--dry-run`** (HARD CONSTRAINT). Priority:
  `edit redo` (one-line; asymmetric with `edit undo`), `rules run --fix`, `kg write` null-delete,
  `rules add/update/remove/setup`, `ratchet`/`budget` CRUD, `sessions mark/unmark`,
  `structure rebuild/packages`, `daemon start/stop`, `generate client/cli-snapshot`, `update`.
  Decide whether idempotent rebuilds get a documented exemption (amend the constraint if so).
- [x] **T1-3: all `trend` metric commands broken** — temp worktree's stale `[embeddings]` config
  hard-aborts validation. Confirm spawn-path root cause; ignore unknown config in spawned worktrees.
- [x] **T1-4: `structure packages` silently succeeds with zero output** (HARD CONSTRAINT). DONE
  (output half): `PackagesReport::format_text` now prints an explicit "No package ecosystems
  detected" message when empty instead of a bare line; `--json` emits `{"ecosystems": []}`.
  FOLLOW-UP: still needs `--dry-run` (writes global cache; counts toward T1-2).
- [x] **T1-5: `rules show <id>` lookup bug** — DONE: `show_rule`/`show_rule_structured` now search
  the native-rules registry too (new `native_rule_info` helper), matching what `rules list`
  enumerates, so `stale-summary` et al. resolve. JSON errors now structured via server-less.
- [x] **T1-6: broken guides + stale `kg --help` examples** — `guide analyze`/`guide rules` reference
  pre-rename `analyze *` paths; `kg --help` references nonexistent `kg create/link/query/show`. Fix
  strings + add a `guide test`/snapshot parsing guide bodies against the real command tree.

**Tier 2 — correctness / consistency:**
- [ ] **T2-1: `--jsonl` doesn't unwrap inner arrays** — object-wrapping reports (`structure files`,
  `rank complexity`, `rules list`, `grep`) emit the wrapper as one line. Add a `jsonl_items()` hook.
- [x] **T2-2: service-layer errors are plain text under `--json`** — DONE (generic path): the
  server-less CLI `Err` arm now emits `{"error": "<msg>"}` on stdout (exit non-zero) under
  `--json`/`--jsonl`/`--jq` for all commands, so `docs`, `view references/referenced-by/trace`,
  `serve mcp`, and service-layer failures get a parseable error object. (Clap-level
  missing-arg/validation errors fire before the service runs and are still plain text — separate
  server-less concern.)
- [ ] **T2-3: `budget --base-ref`→`--diff-ref`** — keep rename, add CHANGELOG entry (see resolved
  `#[param(name)]` item above). Separable: unify git-ref flag taxonomy (`--diff` / `--diff-ref` /
  `--baseline-ref` / positional `[base]`).
- [ ] **T2-4: `--ignore-case` (grep) vs `--case-insensitive` (view family)** — canonicalize on
  `--ignore-case`.
- [ ] **T2-5: `--limit` drift** — `-l` vs `-n` short form; long-only `--n` (ngram size);
  `--top`/`--worst` aliases; `syntax ast -l` = `--at-line`. Canonicalize on `-l, --limit`.
- [ ] **T2-6: `rank budget` collides with `budget` service** — rename to `rank line-breakdown`/`rank purposes`.
- [ ] **T2-7: `cfg cfg` redundant double-wrapping** — collapse to `normalize cfg <path>`.
- [ ] **T2-8: git ref/target as positional vs flag** — `view trace --target`, `skeleton-diff [base]`
  deviate from the prevailing form.
- [ ] **T2-9: inverted dry-run default** — `edit extract-function`, `context migrate` require
  `--apply`. Either flip to write-by-default + `--dry-run`, or document `extract-function` as a
  deliberate-safety exception (recommended: document the exception).
- [ ] **T2-10: `edit history` vs `view history`** — rename `edit history` → `edit log`/`edit trail`.

**Tier 3 — polish:**
- [ ] **T3-1: root-global flag noise** (~9 server-less globals on every leaf `--help`). server-less
  rendering concern — collapse into a `[global options]` footer. Broader than (but related to) the
  already-tracked "Root-global `--pretty` advertised-no-op" item above.
- [ ] **T3-2: help-text gaps** — `rules remove/update` no examples; `analyze all` no body + opaque
  scope; `analyze activity/repo-coupling/cross-repo-health` no examples; `syntax ast --compact`
  truncated description; `analyze` category labels don't scan.
- [ ] **T3-3: short-flag overloading + clashes** — `-d`/`-t` overloaded; `syntax ast --compact`
  clashes with global `--compact` (rename to `--outline`); `view blame --sessions` → `--sessions-dir`.
- [ ] **T3-4: soft feature overlaps** — `analyze architecture`≈`view graph`,
  `analyze coupling-clusters`≈`rank coupling`. Add "see also" or merge.
- [ ] **T3-5: unactionable errors** — `grep <nonexistent path>` silent "no matches";
  `analyze complexity` doesn't hint `rank complexity`.

### server-less UX issues — ~~all fixed~~ (server-less commit 9c294b2)

1. ~~**`name` attribute ignored for nested services**~~: Fixed — `#[cli(name = "...")]` now works on individual methods (leaf and mount). `get_cli_name()` helper added.
2. ~~**No error for helper methods in `#[cli]` block**~~: Fixed — added `#[cli(helper)]` as a self-documenting alias for `#[cli(skip)]`. Module docs updated.
3. ~~**`display_with` across impl blocks is non-obvious**~~: Fixed — module docs now explicitly document that `display_with` functions can live in any impl block on the same type.

### ~~Session analysis bug~~ (already fixed)

~~**Bug: `Turn::token_usage` only captures the last API call per turn.**~~ Already fixed in claude_code.rs — `turn_request_ids: Vec<String>` accumulates all request IDs and `sum_turn_tokens` sums them on flush.

### ~~Daemon memory leak — 2.3GB resident after 10 days~~ FIXED

~~The daemon (`normalize daemon run`) accumulates ~2.3GB resident memory over time. Root cause:
`WatchedRoot` holds `DiagnosticsCache` (all syntax/fact/native issues) and `rev_deps`
(`HashMap<PathBuf, HashSet<PathBuf>>`) **in memory forever** — no eviction.~~

Fixed: `DiagnosticsCache` removed from `WatchedRoot` — diagnostics are now serialized to JSON
and persisted to the `daemon_diagnostics` table in the SQLite index, then dropped from heap
immediately after each refresh. `rev_deps` removed — reverse-dep graph is now derived
transiently from the SQLite `imports` table on each refresh cycle and discarded after use.
`last_affected` was already transient (local to the refresh). `WatchedRoot` now holds only
watcher handles and a `primed: bool` flag — near-zero steady-state memory footprint.

Accumulation of per-root indexes is the next chapter — see the P1 "Content-addressed indexer
(CA store)" entry below.

Remaining (not blocking the memory fix):
- [x] Grammar/tree lifetime: eliminated duplicate `GRAMMAR_LOADER` singleton from
  `normalize-facts/src/parsers.rs` — it now delegates to the canonical singleton in
  `normalize_languages::parsers`; trees are already local and dropped after extraction
  in all call sites; no `GrammarLoader` is stored in daemon state or long-lived structs

### LSP diagnostics improvements

- [x] Per-file syntax rules (only re-run on the saved file, not the whole workspace)
- [x] Incremental index update on save via `FileIndex::update_file()`
- [x] Two-tier diagnostics: immediate syntax, debounced (1500ms) fact rules
- [x] Daemon calls `incremental_call_graph_refresh()` after detecting changes
- [x] Persistent `SkeletonExtractor` in LSP backend (avoids recreating per request)
- [x] Compiled query caching in `GrammarLoader` (tags, imports, calls, complexity)
- [x] Configurable debounce interval (`[serve] fact_debounce_ms`, default 1500)
- [x] **Per-file daemon storage shape**: `daemon_diagnostics_per_file (path, issues_blob, updated_at)` table exists; `filter_files` on `RunRules` request wired; `.normalize/diagnostics.json` written atomically after each refresh. All three pieces confirmed implemented.
- [x] **Daemon-push diagnostics for LSP**: `Event::DiagnosticsUpdated { root, updates }` is now
  broadcast on every prime/refresh with per-file deltas (blob-compared to skip unchanged files).
  Subscribe connections opened with the `0x01` magic byte stream length-prefixed rkyv frames;
  `DaemonClient::watch_events_binary` decodes them. Initial-subscribe replay (item 4) and LSP
  client migration are deferred follow-ups.
- Incremental Datalog for fact rules — **blocked on ascent-interpreter upstream** (ask user for status periodically)
  Agreed roadmap (with ascent-interpreter maintainer):
  1. String interning (makes most values u32)
  2. Flat tuple storage (relations store typed arrays, arity-specialized `[u32; N]`)
  3. **Incremental evaluation** — file-scoped retraction, strata invalidation, persisted engine.
     This is the highest-value step for LSP: process 50 changed facts instead of re-evaluating 500k.
     After interning, diffing facts is just comparing `[u32; N]` arrays → cheap file-scoped retraction.
  4. Bytecode for expressions (measure after step 2 — joins may dominate, not expression eval)
  5. Arity-specialized eval routines (generic over `[u32; N]`, stamped out via macro)
  6. Cranelift JIT (feature-gated; defer decision until after step 4)
- [x] File-level dependency tracking (import graph edges to scope fact re-evaluation) — `WatchedRoot.rev_deps` in daemon, `affected = changed ∪ reverse-deps`, `last_affected` stored for Datalog integration
- [x] `normalize watch` CLI (expose daemon file-watching with TUI output)
- [x] Progress reporting for `structure rebuild` (indicatif bars for file scan, symbol parsing, index storage)
- [x] Progress reporting for `analyze duplicates`, `analyze architecture`, `analyze duplicate-types` (indicatif bars for file processing, spinners for architecture phases)
- [x] Native rules published to LSP clients (missing-summary, stale-summary, check-refs, etc.) — debounced workspace-wide, re-triggered on `.git/index` changes (git add events)
- [x] **Live-reload `.normalize/config.toml` and `.normalize/rules/**`**: fourth dispatch route in the daemon notify loop; on edit, clear `daemon_diagnostics` + `daemon_diagnostics_per_file` and reprime. Subscribers get `IndexRefreshed { files: 0 }` then `DiagnosticsUpdated`.
- [x] **Cross-daemon-restart cache validity (config_hash gate)** — `daemon_diagnostics` / `daemon_diagnostics_per_file` now carry a `config_hash` column (binary version + `.normalize/config.toml` + `.normalize/rules/**`). Mismatch on load = cache miss; daemon reprimes. Schema v10 → v11. Future per-rule cache hashing is unblocked but not yet wired up.
- [x] **Tier 1 surgical config invalidation** — filter-only `.normalize/config.toml` changes (severity bump, allow-list edit, `enabled = false`) no longer trigger a full reprime. The daemon computes a `ConfigDiff` (in `normalize-rules-config`); filter-only diffs flip a `serve_filter_pending` flag and the serve paths re-filter cached blobs in place. Tier 2 (per-rule re-evaluation) and Tier 3 (smart walk-exclude diff) are follow-ups below.
- [x] **Tier 2 surgical config invalidation (per-rule re-eval)** — when only rules' behavior changed (newly-enabled, threshold/extra-field changed), only those rules re-run. The daemon now routes `ConfigDiff::rules_to_rerun` through `surgical_rerun_rules`: re-runs each affected rule through syntax/fact/native engines using existing `filter_ids` params, splices updated findings into per-engine blobs, rebuilds "all" blob and per-file rows, and broadcasts `DiagnosticsUpdated`. `.scm` file hash tracking is also complete: `WatchedRoot` stores `cached_scm_hashes: HashMap<PathBuf, [u8; 32]>` (blake3 per `.normalize/rules/*.scm` file); `reload_config_and_reprime` diffs old vs new hashes, identifies changed rule IDs by file stem, and unions them into `ConfigDiff::rules_to_rerun` before the tier decision — so custom rule edits now route through Tier 2 instead of Tier 3.
- **Tier 3 smart walk-exclude diff** — `[walk] exclude` changes today force a full reprime. Smart approach: compile old + new exclude matchers, walk the tree once, drop per-file rows for newly-excluded files, run rules on newly-included files, leave the rest alone. Rebuild "all" blob from the per-file table. If the walk ends up dominated by I/O it may not be much cheaper than a reprime in practice — measure before committing.

---

## P1 — Short-term Improvements (coherence / usability)

### Content-addressed indexer (CA store) [IN PROGRESS]

Today the daemon holds a separate in-memory index per watched root. With multiple git
worktrees of the same repo registered, this explodes: 60+ busiless worktrees = 6GB+ RSS,
each holding a near-duplicate index of mostly-identical file content.

Right architecture: memoize derived per-file data (parsed CST, extracted symbols, imports,
calls) keyed by content hash. Aggregate per-root structures (resolved import graph, call
graph) remain per-root but become functions over the CA cache. Sharing across worktrees,
time (reverts), and vendored-duplicate files falls out automatically.

**Step 1 done:** CA cache implementation added — SQLite-backed, keyed by
`(blake3(bytes), extractor_version, grammar)` with LRU eviction and stale-version GC.
(Originally a separate `normalize-ca-cache` crate; inlined into `normalize-facts/src/ca_cache.rs`
since it had only one dependent and no standalone value.)

**Step 2 done:** CA cache integrated into `normalize-facts`: `refresh_call_graph` does a
serial CA pre-pass before rayon par-iter; `reindex_files` checks CA cache per file.

**Step 3 done:** Daemon watchers consolidated — single `RecommendedWatcher` + one
dispatch thread in `DaemonServer`; `WatchedRoot` no longer holds watcher handles;
`add_root`/`remove_root` watch/unwatch via the shared watcher.

Short-term mitigations already landed: skip auto-add of git worktrees in
`maybe_start_daemon`; GC dead roots on daemon startup. These stop the bleeding but don't
fix the underlying duplication.

Related: P3 candidate "TTL/LRU eviction for idle roots" — general hygiene, lower priority
once CA store exists.

### Refactoring recipe ecosystem (high priority)

The goal says "rename, find-references, extract, inline, move" but only `rename` exists as a
high-level recipe. `normalize-refactor/src/lib.rs` explicitly lists `move.rs` and `extract.rs`
as future work — they don't exist.

This matters beyond CLI usability: normalize is meant to be the substrate for agent-driven
code editing (e.g. nanites). Without a recipe library, every agent reinvents the same
transformations incorrectly from the Editor primitives. The recipes are the shared correct
implementation nobody should have to re-derive.

Target recipes (in rough priority order):
- [x] `extract_function` — `normalize edit extract-function <file> --lines <start>-<end> --name <name> [--apply]` — CFG liveness–based extraction. The first attempt (commit `ed9d3b63`, reverted) used a tree-sitter identifier sweep; this implementation queries the facts index for CFG data, runs backward-dataflow liveness over the whole function, and derives parameters (live-in to the region from outside) and return values (live-out from the region to later code). Checks `cfg_effects` for async/generator/defer/acquire; emits warnings for semantics that may not transfer (defer crossing boundary, unbalanced resource lifetime, escaping exception edges). Generates language-appropriate source for Rust, Python, Go, TypeScript/JavaScript, Java. Dry-run by default. Recipe at `crates/normalize-refactor/src/extract_function.rs`.
- [x] `inline_variable` — inverse of extract: replace all uses of a variable with its initializer and remove the binding (`normalize edit inline-variable <file> <line>:<col>`, recipe at `crates/normalize-refactor/src/inline_variable.rs`). Position points to the variable name in its declaration. Supports Rust, TypeScript/JavaScript, Python. Errors on reassignment or missing initializer; warns on side-effect risk with multiple references. `--safe` flag refuses to inline unused variables.
- [x] `inline_function` — `normalize edit inline-function <file> <line>:<col>` — inlines a single-use function at its call site within the same file. Substitutes arguments for parameters (whole-word replacement), strips `return` keyword, removes the definition. Supports JS/TS function declarations and arrow `const` bindings, Python `def`, Rust `fn`. Conservative: aborts on multiple-return bodies or mismatched argument counts. `--force` overrides single-use check. Recipe at `crates/normalize-refactor/src/inline_function.rs`
- [x] `move_item` — move function/struct/type to another file, fix imports (`normalize edit move`, recipe at `crates/normalize-refactor/src/move_item.rs`). Best-effort import rewriting for Python/Go/JS/TS; Rust and unsupported cases emit warnings rather than fabricate paths. `--reexport` available for Python.
- [x] `add_parameter` / `change_signature` — update function signature + all callsites (`normalize edit add-parameter <file> <function> --param <name> --default <value> [--type <type>] [--position <N>]`, recipe at `crates/normalize-refactor/src/add_parameter.rs`). Uses tree-sitter to locate the function and argument lists. Finds all call sites via the facts index; falls back with a warning if the index is unavailable. Supports Rust, TypeScript/JavaScript, Python.
- [x] `introduce_variable` — extract expression into a named binding (`normalize edit introduce-variable <file> <range> <name>`, recipe at `crates/normalize-refactor/src/introduce_variable.rs`). Language-specific binding keyword: Python uses bare assignment, JS/TS use `const`, all others use `let`. Range specified as `start_line:start_col-end_line:end_col` (1-based).

Each recipe should be language-agnostic where possible (via the Language trait + .scm queries)
with language-specific overrides for things the generic tree-sitter model can't express.

#### Semantic foundation needed

Correct recipes (especially extract/inline) need semantic infrastructure normalize doesn't have:

1. **Name resolution** — ~80% there via `locals.scm`. Gap: cross-file module-level resolution. Builds on the facts index. Tractable.
2. **Control flow graph** — Phase 1+2+3 complete. `.cfg.scm` queries for 76 languages; `BasicBlock` with `DefSite`/`UseSite`/`effects`; SQLite persistence including `cfg_effects`; Datalog relations. See CFG section above.
3. **Liveness analysis** — Phase 2 complete. `liveness.dl` builtin Datalog rule; `normalize analyze liveness <file> --function <name>` CLI command. Backward-dataflow fixed-point over CFG blocks from the index.
4. **Effect/mutation tracking** — Phase 3 complete for structural effects (await, defer, yield, acquire/release, send/receive). `normalize analyze effects <file>` command. Precise mutation tracking (Rust `&mut` vs `&`) still needs compiler integration.
5. **Type information** — tiered strategy:
   - **Tier A (in-house):** Syntactic extraction from declarations (struct fields, function signatures, typed lets). Mechanical.
   - **Tier B (in-house):** Type-flow across the call graph — `let x = foo()` resolves to `foo`'s declared return type. Datalog-friendly once Tier A exists.
   - **Tier C (LSP delegation):** Query language servers for type-at-position when tiers A+B can't resolve. Pragmatic; not ideological. Accepts runtime dependency on LSPs for the hard cases.
   - **Tier D (warnings/placeholders):** When even LSP fails, emit placeholders (`_` in Rust, `any` in TS) and surface warnings.

Per-language difficulty:
- **Tractable in-house (tiers A+B sufficient):** Go, Java, C, dynamically-typed langs (Python/Ruby/JS — no types needed)
- **Full HM languages (OCaml, Haskell-minus-extensions, Rust's core type system):** HM is well-trodden literature; implementable in-house if we commit to it
- **Research-grade hard (LSP delegation recommended):** TypeScript (conditional/mapped/template-literal types), C++ (templates, SFINAE, concepts), Scala (implicits, path-dependent types), Rust trait resolution under generics (the machinery, not the types)

This is an epic, not a drive-by. Do the tractable recipes (`move_item`, `add_parameter`, `inline_variable`) first — they expose less of the semantic gap — then build the foundation (1→2→3→5), then revisit extract/inline with real scope and type info.

### ~~Eliminate remaining git shell-outs (budget metrics worktrees + ratchet ref-based check/measure)~~ DONE

All budget metrics now use gix in-memory blob reads — no filesystem checkout, no `git` binary
required. `lines`, `modules`, `todos`, `dependencies` use `diff_tree_to_tree` comparing
base_ref to HEAD. `functions`, `classes`, `complexity-delta` use `walk_tree_at_ref` to read
blobs from the object store for the base tree, then read working tree from disk.
Committed: `refactor(budget): replace git worktrees with in-memory gix blob reads`.

Ratchet ref-based check/measure (`--baseline-ref`, `--diff-ref`) also migrated: now uses gix
blob reads via `walk_tree_at_ref` materialised into a tempdir, replacing `git worktree add/remove`.
No `git` binary in PATH is required for any normalize operation.
Committed: `refactor(ratchet): replace git worktrees with in-memory gix blob reads`.

### ~~Migrate remaining read-only git shell-outs (blame, status, path-log)~~ DONE

Remaining read-only shell-outs migrated to gix:
- `git blame` → `repo.blame_file()` (ownership.rs, provenance.rs, view/history.rs)
- `git status --porcelain` → `repo.status().into_index_worktree_iter()` (git_utils.rs, stale_summary.rs, sources.rs)
- `git log -- path` (path-filtered count/last-commit) → commit walk with tree diffs (git_utils.rs, stale_summary.rs)
- `git rev-list --count HEAD` → commit walk length (service/analyze.rs coupling_clusters)
- `git rev-parse` shell-out (`resolve_ref_shellout`) → delegates to `resolve_ref()` gix wrapper
- `git log --name-only` (co-change) → existing `git_per_commit_files()` gix helper (provenance.rs)
- `git diff --cached` / `git status` in GitSource → `repo.is_dirty()` + index-vs-HEAD blob compare
Committed: `refactor: migrate remaining git shell-outs to gix (blame, status, path-log)`.

### Agent UX: comprehensive compact output audit

Baseline audit in `docs/agent-ux-audit.md` (2026-03-21) covers 12 commands across 3 models (Haiku, Sonnet, Opus). Quick wins fixed. Remaining work:

- [x] `analyze health --json` and `analyze summary --json` bloated by unbounded `large_files` (180KB+) → added `--limit` (default 10, 0 = no limit) to `analyze health`
- [x] `syntax ast --json` produces 673KB for a 200-line file → added `--depth` flag (default -1 = unlimited) that truncates the CST; `--compact` now shows a node-type outline instead of full dump
- [x] `syntax query --compact` only showed match count → now shows one line per match: `path:line: @capture = text`
- [x] `rules show --json` and `rules tags --json` returned `{message: string, success: bool}` text blobs → now return `RuleInfoReport` (structured fields: id, severity, enabled, tags, languages, message, fix, description, allow) and `RulesTagsReport` (`{tags: [{tag, source, count, rules}]}`)
- [x] `daemon list` returned exit 1 + stderr when daemon not running → now returns exit 0 with `{running: false, roots: []}` (only exits 1 on actual errors like socket permission denied)
- [x] `package list --json` silently dropped multi-ecosystem advisory → `PackageListReport` now includes `ecosystems_detected: Vec<String>`; when multiple ecosystems exist, agents see all names and know to re-run with `--ecosystem` for complete results
- [x] `analyze architecture --compact` showed only cross-imports (hubs/layers hidden when empty) → `format_text()` now emits compact tagged lines (`HUBS:`, `LAYERS:`, `COUPLING:`, `SYMBOLS:`, `ORPHANS:`, `SUMMARY:`) always, even when empty; `format_pretty()` retains the original tabular layout
- [x] Run full audit pass over all ~30+ subcommands — Pass 2 committed 2026-03-26, 37 commands evaluated
- [x] Re-run multi-model audit after fixes to verify improvement — Pass 2 in `docs/agent-ux-audit-2.md` (2026-03-27): all 8 fixes confirmed; 6 remaining issues filed
- [x] `rules run --only` semantic gap: `files_checked` still counts all files even when `--only` filters output — fixed: `files_checked` recomputed from filtered issues after `--only`/`--exclude` in `normalize-rules/src/service.rs`

### ~~`normalize grep` path scoping~~ (done 2026-03-26)

Added positional `path` arg to `normalize grep`. Also added `--only`/`--exclude` to `normalize rules run` and `normalize structure rebuild`. Fixed pre-existing build errors: missing `pub mod rename` declaration, `build_view_service`/`build_view_list_service` missing `context_files` arg, `build_stale_summary_report` missing filenames/paths args.

### ~~Main Crate Responsibility Boundaries~~ (audited 2026-03-15 — no action needed)

Crate split is correct. All 38 published crates justified. No reusable logic trapped in `normalize`; no unjustified extractions. Single-consumer domain libraries (graph, scope, edit, deps, etc.) are correctly placed — the test is "CLI wiring vs. domain logic", not "has 2+ consumers". Revisit only if a concrete second consumer appears for a specific module.

### Multi-language `normalize docs`

- [x] **`normalize docs` is now multi-language (Rust/Go/Python)** (2026-05-29).
      Previously Rust-only (docs.rs). Now dispatches generically over the
      `Ecosystem` trait (`docs_extractor`/`docs_fetcher`/`package_from_symbol`/
      `docs_language`) with an `--ecosystem`/`-e` flag (auto-detect when
      omitted). Shared primitives `source_archive` (download+extract registry
      archives) and `doc_tree` (tree-sitter docstring/signature extraction)
      back Go (mod cache local + module-proxy `.zip` remote) and Python
      (site-packages local + PyPI sdist remote). Doc bodies are source-native
      (`doc_body` + `doc_format`); rendering moved to the output layer
      (`render_symbol_doc`). KG cache prefix changed `docs-cargo-`→`docs-rust-…`.
      This effort was not previously tracked here.

Follow-ups: see Deferred — `DocFormat`-aware rendering, Rust source-archive unification, `escape_go_proxy` bug, remaining-ecosystem doc support.

### Analyze Command Consolidation — remaining work

**Current: ~35 commands** (after 2026-03-15/16 consolidation: deleted `analyze parse`, `analyze query`, `analyze all`, `analyze node-types` → moved to `syntax`; merged 4 trend commands; deleted `normalize-rules-loader`). Trend commands moved to `normalize trend` on 2026-03-28 (5 methods removed from analyze, 5 added to trend service). `analyze length` and `analyze test-gaps` moved to `rank`; `analyze node-types` duplicate removed (2026-03-28).

**Phase 3 rank infrastructure (done 2026-03-12):**
- `RankEntry` trait + `Column`/`Align` + `format_ranked_table()` in `normalize-rank::ranked`
- Migrated 13 commands to shared tabular rendering
- `DiffableRankEntry` + `--diff` on all 12 rank commands

**Future (low priority):** `security` → SARIF rules engine. `docs`/`security` → rules migration (~-3 commands).


**T1-6 fixed (2026-06-29):** `kg --help` footer examples updated from nonexistent
`kg create`/`kg link`/`kg query`/`kg show` to actual `kg write`/`kg read`/`kg walk`.
Guide content in `guide.rs` updated: `analyze complexity/length/duplicates/ceremony/size/density`
→ `rank` equivalents; `analyze node-types` → `syntax node-types`; `analyze graph/dependents/call-graph`
→ `view graph`/`view dependents`/`view references`; `analyze complexity-trend` → `trend complexity`.
Audit doc: `docs/artifacts/cli-audit-2026-06-29/05-command-structure.md`.

**T2-3 documented (2026-06-29):** `budget measure` and `budget add` `--base-ref` → `--diff-ref`
rename logged in CHANGELOG.md [Unreleased]. The rename was a consequence of server-less 0.6 work;
ref-flag taxonomy unification (rest of T2-3) is deferred — no blocking issues.

**Guide regression guard (now scheduled as retree R0):** A test that validates guide
command examples exist prevents T1-6-class regressions. It is now the **first** step of the
CLI full-retree (must land before any rename) — see "CLI command-taxonomy FULL RETREE" below
and plan §6. Harness: parse each guide's `const &str` body for `normalize …` lines, resolve
each against the live command tree, fail CI on any unresolved example.
---

### CLI command-taxonomy FULL INVERSION — seam-corrected final scope (high priority)

**Decision made (2026-06-30): full inversion by CRATE OWNERSHIP** — push the `#[cli]`
service DOWN into the owning compute crate so the top-level verb *is* the crate (per
CLAUDE.md's crate-owns-its-CLI rule). **Supersedes the 2026-06-29 output-shape retree.**
Authoritative plan (FINAL SCOPE section is canonical; body is supporting evidence):
`docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md`. Seam evidence:
`docs/artifacts/cli-taxonomy-2026-06-29/seam-evaluation.md`. Cross-refs audit T2-6
(`rank budget`), T2-7 (`cfg cfg`), T2-10 (`edit history`), T3-4 (architecture near-dup),
T1-6 guide-regression.

**Architecture extractions (decided, independent of verb taxonomy):**
- `normalize-git` — verbatim `open_repo`/`read_blob_text`/`walk_tree_at_ref`/
  `traverse_tree_entries` duplicated across budget/ratchet/semantic/native-rules + `blame_file`
  duplicated within main crate. 6+ actual dependents = duplication bug today, not speculation.
  Extract `git_utils.rs` as new crate; migrate all dependents in same batch (B1).
- `normalize-git-history` — hotspot/coupling/bus-factor/activity analysis is a recognized
  standalone tool category (code-maat/git-of-theseus). Passes standalone-useful criterion.
  Extract typed data API; OutputFormatter stays in main crate. Depends on normalize-git.
  Becomes the `history` verb (B8 extraction, B9 mount).
- Fold cyclomatic-complexity tags-walking from main `analyze/complexity.rs` into
  `normalize-facts::extract` (dedup of the `compute_complexity` wrapper; not a new crate; B5).

**Metric core stays A1 — confirmed by seam evaluation (no metrics crate):**
Complexity, length, ceremony, density, imports, surface, size, files, test-ratio, test-gaps
have no coherent compute crate. Two disjoint dependency groups: AST-group (no SQLite) and
index-group (entangled with `crate::index::FileIndex`). A `normalize-metrics`-AST crate
would collide with the existing ratchet `normalize-metrics`, have one dependent, duplicate
`compute_complexity`, and exist solely to back a verb. **`rank` and `trend` stay main-crate
verbs.** CI lint (RankEntry-based, B11) holds against future drift.

**Reachable crate-owned verbs:** `graph` (normalize-graph), `architecture`
(normalize-architecture), `similarity` (normalize-code-similarity), `structure`
(normalize-facts — mount real service, drop main-crate dup, absorb dataflow trio), `filter`
(normalize-filter), `search` (normalize-semantic — wire the orphan), `history`
(normalize-git-history). Kept: `budget`/`cfg`/`kg`/`ratchet`/`rules`. Verb names confirmed:
architecture, graph, similarity, structure, search, filter, history, overview.

**Blast radius (total):** 2 new crates (normalize-git, normalize-git-history); 5 crates
gain `cli` feature (graph, architecture, code-similarity, facts, semantic); 6+ crates
migrated to normalize-git; ~20 commands re-pathed (~12% of ~165); ~5 renames.

Implementation order (each batch: build + `cargo test -q` green; docs synced same commit):
- [ ] **B0 — gates + server-less prereq:** guide/help regression test (parse guide bodies,
  assert every `normalize …` example resolves) + cli-snapshot topic test; fix CLAUDE.md
  "38 crates"→"47"; add `#[cli(alias = "...")]` to server-less + publish + bump dep.
- [x] **B1 — `normalize-git`:** extract `git_utils.rs` as new crate; migrate ALL dependents
  (budget, ratchet, semantic, main crate); budget/ratchet git_ops.rs → thin re-export wrappers;
  semantic git_staleness.rs uses normalize_git::open_repo; main git_utils.rs → pub use normalize_git::*.
- [x] **B2 — `graph`:** ✅ DONE 2026-07-03. Added `cli` feature to `normalize-graph`
  (gates `report`+`service` modules; pure algorithms build with `default-features = false`).
  Moved report structs (`GraphReport`/`DependentsReport`/`GraphStats`/`GraphTarget`/
  `ImportPathReport`), `assemble_graph_report`, graph construction, and `OutputFormatter`
  impls out of `crates/normalize/src/commands/analyze/{graph,import_path}.rs` (deleted) into
  `normalize-graph/src/report.rs`. Added `GraphService` (`service.rs`): owns config access
  (loads `[index]`/`[walk]`/`[pretty]` slices standalone, acquires via
  `normalize_index::require_import_graph`). Mounted as top-level **`graph`** verb (default =
  module graph; `graph dependents`; `graph import-path`). Old `view graph`/`dependents`/
  `import-path` kept as **hidden** transitional shims on `ViewService` (`#[cli(hidden)]`)
  delegating to `normalize_graph::*` for one release. Build matrix (default / `cli` /
  all-features / no-default-features) green, no dep cycle (graph→index→facts, acyclic).
  clippy + tests green (CLI help snapshots updated). **Main-crate `src` LOC: −1156.**
- [x] **B3 — `architecture`:** ✅ DONE 2026-07-03. Added `cli` feature to
  `normalize-architecture` (gates `architecture`/`layering`/`depth_map`/`output`/`service`
  modules; pure algorithms build with `default-features = false`). Moved the 3 report structs
  + compute (`ArchitectureReport`/`analyze_architecture`, `LayeringReport`/`analyze_layering`,
  `DepthMapReport`/`analyze_depth_map`) + `OutputFormatter` impls out of
  `crates/normalize/src/commands/analyze/{architecture,layering,depth_map}.rs` (deleted) into
  the crate; recreated a `crate::output` re-export (adds a local `pretty_ranked_table`) to keep
  the moved code unchanged. Added `ArchitectureService` (`service.rs`): owns config access
  (loads `[index]`/`[walk]`/`[pretty]` slices standalone, acquires via
  `normalize_index::{require_import_graph, ensure_ready}`; `--diff` baseline via `normalize-git`
  `resolve_ref`/`run_in_worktree` + `normalize_rank::compute_ranked_diff`). Mounted as top-level
  **`architecture`** verb (default = coupling/hubs; `architecture layering`;
  `architecture depth-map`). Two-parent transitional shims: `analyze architecture` (on
  `AnalyzeService`) + `rank layering`/`rank depth-map` (on `RankService`) kept as
  `#[cli(hidden)]` delegating to `normalize_architecture::*` for one release; hidden from
  `analyze --help`/`rank --help`, both old+new paths verified working. Build matrix (default /
  `cli` / all-features / no-default-features) green, no dep cycle. clippy + tests green (CLI
  help snapshots updated). **Main-crate `src` LOC: 73704 → 72823 (−881).**
- [x] **B4 — `similarity`:** ✅ DONE 2026-07-03. Added `cli` feature to
  `normalize-code-similarity` (gates `duplicates`/`duplicates_views`/`fragments`/`clusters`/
  `service` modules + report structs, `OutputFormatter` impls, and the filesystem-walking
  compute passes; pure MinHash/LSH/AST-hashing algorithms build with
  `default-features = false`). Moved the 4 command files (`duplicates.rs`,
  `duplicates_views.rs`, `fragments.rs`, `clusters.rs`) out of
  `crates/normalize/src/commands/analyze/` into the crate; rewired `crate::extract` →
  `normalize_facts`, `crate::filter` → `normalize_filter`, `crate::output` →
  `normalize_output`, `crate::parsers` → `normalize_languages::parsers`, and copied
  `is_source_file`/`detect_project_languages`/`build_filter` into the crate. Added
  `SimilarityService` (`service.rs`): owns config access, loading `[analyze]`/`[aliases]`/
  `[pretty]` slices standalone (tolerant `AnalyzeSlice` via `toml::Value` flatten — no
  `NormalizeConfig` dep). **Index-free** — all compute walks the filesystem directly
  (`ignore::WalkBuilder` + `parse_with_grammar` + `Extractor`), no daemon/import-graph.
  Mounted as top-level **`similarity`** verb (default = duplicates, with `--mode clusters`;
  `similarity duplicate-types`; `similarity fragments`). Transitional shims: `rank duplicates`/
  `rank duplicate-types`/`rank fragments` (on `RankService`) kept as `#[cli(hidden)]`
  delegating to `normalize_code_similarity::*` for one release; hidden from `rank --help`,
  both old+new paths verified working. `health.rs` + `uniqueness.rs` repointed to the crate.
  Build matrix (default / `cli` / all-features / no-default-features) green, no dep cycle.
  clippy + tests green (CLI help snapshots updated). **Main-crate `src` LOC: 72823 → 69087
  (−3736).**
- [x] **B5 — `structure` fix + dataflow:** ✅ DONE 2026-07-03. **Two-service divergence found
  and reconciled:** the main-crate `service/facts.rs` was the *complete* one (rich rebuild with
  include/only/exclude/full/strict/dry-run, stats+storage, files, packages, query,
  test-fixtures ≈ 1331 LOC); the `normalize-facts` `FactsCliService` was a 3-method *stub*
  (simple rebuild/stats/files). Deleting main and mounting the stub would have LOST packages /
  query / test-fixtures / the rich rebuild+stats. So the full service was **ported into the
  crate** (adapted off `NormalizeConfig`: tolerant `[walk]`/`[aliases]` slice loaders +
  `detect_project_languages` + local `build_filter`, mirroring the B4 code-similarity pattern;
  `crate::index::open`→`open_index`, `SkeletonExtractor`→`crate::extract::Extractor`,
  `crate::index::ensure_ready` replicated on `FileIndex` without the `[index] enabled` gate).
  Dead `CommandReport` dropped. Dataflow trio (report structs + `OutputFormatter` + `analyze_*`
  fns) moved into `normalize-facts::service::{liveness,effects,exceptions}`; added
  `structure liveness`/`effects`/`exceptions`; old `analyze` paths kept as `#[cli(hidden)]`
  shims delegating to `FactsCliService` (verified hidden from `analyze --help`, still callable).
  `normalize-facts` `cli` feature activated in main + gained `normalize-filter` dep. Build
  matrix (default / `cli` / all-features / no-default-features) green, **no `facts ⇄ *` cycle**
  (facts stays a leaf). clippy + `cargo test -q` green (CLI help snapshots updated: trio added
  to `structure`, removed from `analyze`). **Main-crate `src` LOC: 69087 → 66839 (−2248 net;
  −2264 deletions).** Dataflow home realized as `structure`. **Deferred to B5-followup (not
  done here):** the cyclomatic-complexity wrapper fold into `normalize-facts::extract`, and the
  parked `cfg liveness` naming alternative (move `normalize-cfg`'s render `CfgService` into
  facts) — neither blocks; revisit if `structure liveness` naming grates.
- [x] **B6 — `filter`:** ✅ DONE 2026-07-03. Mounted `normalize-filter`'s `FilterCliService`
  as the top-level `filter` verb (`#[cli(name)]` renamed `normalize-filter`→`filter`; main-crate
  filter dep gained the `cli` feature). Retired the main-crate `aliases` leaf → now a
  `#[cli(hidden)]` transitional shim (unchanged behavior/format, hidden from `--help`, still
  callable) for one release. **Reconciliation vs plan:** the crate service used
  `AliasConfig::default()` (no config, no language detection) — mounting as-is would have
  regressed custom `@aliases` and language-aware `@tests`. Fixed by giving the service its own
  config-slice loading (`load_alias_config` reads the `[aliases]` table from
  `.normalize/config.toml`) + `detect_project_languages` (via a cli-gated `normalize-languages`
  dep), no `NormalizeConfig` dependency — mirroring the B4/B5 pattern. Both leaves gained a
  `--root`; `filter matches` now resolves aliases too. `AliasesReport` gained a
  `detected_languages` footer. Verified: `filter aliases`/`filter matches` (incl. `--json`/`--jq`)
  work and pick up this repo's `@todo` custom alias + detected languages; old `aliases` works +
  hidden; `search`/`find`→`grep` aliases untouched. Build matrix (default / `cli` /
  all-features / no-default-features) green, no cycle (normalize-languages doesn't depend on
  filter). clippy + `cargo test -q` green (2 CLI help snapshots updated: `aliases`→hidden,
  `filter` added). Also fixed a stale docs example (`[filter.aliases]`→`[aliases]`). **Main-crate
  `src` LOC: 66839 → 66850 (+11: mount wiring; the `aliases` display logic + `detect_project_languages`
  stay in `commands/aliases.rs` because the shim uses them and `build_filter` shares the
  detector — both retire at B12).**
- [x] **B7 — `search`:** ✅ DONE 2026-07-03. Added `SemanticCliService` (`#[cli(name="search")]`,
  `cli` feature) to `normalize-semantic` and mounted it as the top-level `search` verb; the
  single `search` method is `#[cli(default, ...)]` so `normalize search <query>` is a leaf, not
  `search search`. The compute was already present but ORPHANED (`run_search` + `SearchReport`
  with `OutputFormatter` in `service.rs`, no `#[cli]` wrapper, no mount) — the plan's "orphan
  `#[cli]` service" was slightly off (no `#[cli]` existed; only the compute did), so B7 wrote the
  thin wrapper rather than inventing a service. Config wiring is budget-style: `run_search` loads
  the `[embeddings]` slice from `.normalize/config.toml` itself (no `NormalizeConfig` dep); when
  disabled/unpopulated it prints an actionable stderr message + non-zero exit (verified
  non-interactive). **Removed the `search`→`grep` alias** from `main.rs` `rewrite_aliases` + the
  `docs/cli-design.md` aliases table row — `search` now routes to semantic search, not grep
  (proven: `normalize search "…"` hits the semantic "not enabled" path, not a text match);
  `find`→`grep` still works. Also fixed stale `structure search` doc refs in `normalize-semantic`.
  Build matrix (default / `cli` / all-features / no-default-features) green; clippy + `cargo test -q`
  green (1 snapshot updated: `help_root` gains `search`; added `assert_output_formatter::<SearchReport>()`).
  **Main-crate `src` LOC: 66850 → 66863 (+13: mount wiring + output.rs test line).**
- [ ] **B8 — `normalize-git-history` extraction:** define typed data API (`ChurnStats`,
  `CoupledPair`, `OwnershipEntry`, `HotspotEntry`, etc.); disentangle compute from
  `OutputFormatter` in each command file; move compute fns into new crate; OutputFormatter
  stays in main crate.
- [ ] **B9 — `history` verb:** add `#[cli(name="history")]` to normalize-git-history; mount;
  move `rank hotspots`/`rank coupling`/`rank ownership`/`rank contributors`/`analyze
  activity`/`analyze repo-coupling`/`analyze cross-repo-health`.
- [ ] **B10 — syntax-rules consolidation:** confirm `rules run --type syntax` routes syntax
  rules; if confirmed, delete standalone `SyntaxRulesService` CLI from normalize-syntax-rules
  (do NOT add a second verb).
- [ ] **B11 — small fixes + overview + CI lint:** `cfg cfg`→`cfg`; `edit history`→`edit log`;
  `rank budget`→`rank purposes`; dashboards `health`/`summary`/`all`→`overview` (thin
  main-crate composition verb); add RankEntry-based CI lint (flags metric commands drifting
  to wrong verb; rank/trend explicitly allow-listed).
- [ ] **B12 — alias sunset:** remove all hidden transitional aliases/shims at 1.0.
- [ ] **Doc sync (every batch):** `docs/cli/`, `README.md`, `LLMS.md`, `docs/cli-design.md`,
  all guide bodies, `CHANGELOG.md`, touched `SUMMARY.md`s, regenerate `cli-snapshot`.

**Prior-decision history (superseded by the retree above):**
- Done 2026-03-16: `rank` introduced; graph navigation folded into `view`; `ViewOutput`
  dissolved. Done 2026-03-28: trend commands → `trend`; `length`/`test-gaps` → `rank`.
- The old open questions ("where do big-picture commands live", "does analyze dissolve")
  are now answered: `health/summary/all`→`overview`; `analyze` dissolves entirely.

### Graph crate refactor (RESOLVED 2026-07-02)

**Resolved: refactor `normalize-graph` in place — no standalone crate, no node-type
genericization.** Decision record (with the superseding resolution at its top):
`docs/artifacts/cli-taxonomy-2026-06-29/DECISION-graph-crate.md`.

Why the earlier "spin the generic half into its own crate" plan was dropped: a
petgraph/ecosystem survey found **no plumbing gap** — petgraph's `Visitable`/`VisitMap` +
trait-generic algorithms already give bring-your-own-node-type; `pathfinding` gives
closure-based SCC over raw `HashMap`s. `find_longest_chains` is an admitted APPROXIMATE
heuristic (correct DAG-longest-path exists in rustworkx-core), so it stays internal.
Motif/diamond detection is a real ecosystem gap but the coupling cost of an external dep for
one ~50-line algorithm isn't worth it — normalize keeps its own `find_diamonds`. Every caller
uses `String`, so genericizing over node type served only the abandoned standalone ambition.

Done:
- [x] Characterization test suite added to `normalize-graph` (14 tests, deterministic).
- [x] Split presentation out: `GraphTarget`/`GraphStats`/`GraphReport`/`DependentsReport` +
      `OutputFormatter` impls + `assemble_graph_report` moved to
      `crates/normalize/src/commands/analyze/graph.rs`. `normalize-graph` dropped
      `normalize-output` and `nu-ansi-term` deps — now genuinely pure algorithms.
- [x] Dead duplicate `find_longest_chains`/`longest_path_from` already removed from
      normalize-architecture (re-exports the canonical versions).
- [x] Behavior preserved (clippy clean, tests green); B2/B3 unblocked.

Follow-up discovered during the refactor:
- [x] **BUG: `tarjan_sccs` returned all singletons for real cycles** (fixed 2026-07-02).
      Root cause confirmed as reported: the iterative Tarjan pushed its `Frame::Resume(node, "")`
      sentinel AFTER the neighbor frames; the call stack is LIFO, so the sentinel was popped
      FIRST and the SCC-root check (`lowlink == index`) ran before any child `Enter` frame.
      Fix: push the sentinel FIRST so it is popped LAST, after all children propagate lowlink.
      Replaced the buggy characterization test with a full correct-output SCC suite (2-cycle,
      3-cycle, self-loop, nested/multiple SCCs, pure DAG, disconnected cycles, determinism).
      `find_bridges` was audited for the same class of bug and found CORRECT (it uses a
      `last_mut()` + adjacency-cursor pattern with post-processing on pop) — added a
      bridge-vs-cycle-edge correctness test proving so.
- [ ] **Real circular dependency now surfaced by `view graph` (module graph):** a 28-module
      SCC inside `crates/normalize/src/` — the service layer (`service/mod.rs`, `service/view.rs`,
      `service/config.rs`, `service/context.rs`, `service/package.rs`, `service/sessions.rs`, …)
      tangled with `output.rs`, `config.rs`, and `index.rs`. This was invisible while
      `tarjan_sccs` was broken. Fold into the main-crate decomposition audit below when
      untangling the service layer; the mutual `output.rs` ↔ command/service coupling is the
      likely knot.

### Main-crate decomposition audit (DONE 2026-07-02)

✅ **Full audit complete — findings in `docs/audit-2026-07-02.md`.** Five cluster audits
(similarity, sessions, CFG dataflow, scoring/extraction, service layer) run against the
strict crate bar (multiple actual dependents OR genuine standalone value).

**Headline:** the main `normalize` crate is NOT a reservoir of extractable domain logic.
The reusable algorithms (MinHash/LSH, cyclomatic complexity, symbol extraction, model
pricing, refactor transforms, liveness solver) are already in feature crates. The ~84k
LOC is CLI-surface breadth, not leaked domain logic — this partially refutes the handoff
premise. The March P2 "extract analysis algorithms" item is effectively already
satisfied; only small dedups/extractions remain.

Explicit non-extractions and latent notes are recorded in the audit doc so they are not
re-litigated (keep both session crates; `normalize-cfg` stays; `health.rs` /
`skeleton.rs` / `extract.rs` / `service/edit.rs` / `service/facts.rs` core all stay as
legit wiring; latent: dual "parallelization savings" vocab, `module_health.rs` vs
`health.rs` scorer overlap).

**Correction (2026-07-02) — vendored CLIs are not a size lever.** The ~26% of the main
crate that is vendored full-CLI front-ends (`src/rg/` ~13.9k, `src/ast_grep/` ~7.0k,
`src/jq/` ~0.7k; ~21.6k lines) measures **line-count, not cost**. The engines
(`jaq-*` / `grep`+`ignore` / `ast-grep-*`) are sunk cost — already linked for the
first-class `--jq` / `normalize grep` / ast-grep features — so the marginal cost of the
vendored front-ends is ~zero and extracting them would **not** shrink binary/compile cost.
The marginal-cost rationale is now documented in `docs/cli-dropin-integrations.md`
("Why the line count is not the cost") and the audit framing is corrected in
`docs/audit-2026-07-02.md`.

- [x] **FORCED 2026-07-02 — vendored CLIs stay in main; extraction is IMPOSSIBLE, not just
  rejected.** A genuine trilemma: you cannot have all of (1) purity (vendored CLI source out
  of `normalize`), (2) publishable-with-drop-ins (`cargo install normalize` ships `rg`/`jq`/`sg`),
  and (3) no junk crates (no verbatim third-party CLI copies on crates.io). The vendored code
  is CLI *source* upstream publishes only as binaries (engines are libs, front-ends aren't), so
  a published `normalize` carrying the drop-ins must hold that source in-crate (breaks 1) or in
  a published dep (breaks 3). `publish = false` crates don't escape it — a published crate
  cannot depend on one (path dep fails cargo's version check; versioned dep fails registry
  validation; both verified this session); a `publish = false` multitool *binary* fixes the dep
  but breaks (2). The project chose (2)+(3), which forecloses (1) → keeping them in main is
  forced. Secondary (weaker): also fails the crate bar (one dependent, zero standalone value,
  no coherent `normalize-*` name). Marginal cost is ~zero (engines sunk). Former
  version-lockstep / publishing gates are **moot**. Full reasoning: `docs/audit-2026-07-02.md`
  ("Decision (2026-07-02): keeping the vendored CLIs in main is FORCED by a publish trilemma")
  and `docs/cli-dropin-integrations.md`.

**Execution items (plan of record — executed this session in subsequent commits):**

- [x] **D1 — liveness dataflow → `normalize-facts`** (`facts::cfg_dataflow`): ✅ DONE 2026-07-02.
  New module `crates/normalize-facts/src/cfg_dataflow.rs` exposes `compute_liveness()` +
  `load_blocks`/`load_edges`/`load_defs`/`load_uses` (rows `CfgBlockRow`/`CfgEdgeRow`).
  `commands/analyze/liveness.rs` and `normalize-refactor/src/extract_function.rs` both rewired to
  it; the near-verbatim `compute_liveness` copy + 4 duplicated loaders deleted from refactor.
  `BlockLiveness`/`LivenessReport` + OutputFormatter stayed in main. Pure dedup, user-invisible.
- [x] **D2 — LSH candidate-pair loop → `normalize-code-similarity`** (2026-07-02): added
  `lsh_candidate_pairs(&[[u64; MINHASH_N]]) -> Vec<(usize, usize)>` (parallel per-band
  bucketing + merge/dedup, rayon) beside `lsh_band_hash`, with unit tests. Rewired all 3
  call sites (`duplicates.rs` functions + blocks pipelines, `fragments.rs::group_fuzzy`);
  command-specific scoring/thresholding stays in main. The blocks site's serial single-map
  variant now shares the parallel path — same candidate set. Pure dedup, user-invisible.
- [x] **D3 — dedup hand-rolled `UnionFind`** (2026-07-02): deleted the local `UnionFind`
  struct in `clusters.rs` and the inline `find`/`union` fns in `duplicates.rs`
  (`suppress_widespread_body_patterns`), routing both onto
  `normalize_code_similarity::UnionFind`. Also repointed `coupling_clusters.rs` (was
  importing the clusters-local copy). Crate API (`new`/`find`/`union`) covered every
  caller — no extension needed. Same semantics (union-by-rank + path compression).
- [x] **D4 — `aggregate_sessions` fold → `normalize-session-analysis`**: ✅ DONE 2026-07-02.
  Fold moved into the crate as inherent method `SessionAnalysisReport::aggregate(&[SessionAnalysisReport])
  -> SessionAnalysisReport` (behavior preserved; unit tests added). Main's `aggregate_sessions`
  now parses paths into reports and delegates; print/dispatch wrapper stays in main.
- [x] **D5 — dedup heatmap `normalize_path`**: ✅ DONE 2026-07-02. Byte-for-byte copy in
  `commands/sessions/heatmap.rs` deleted; now imports the canonical
  `normalize_session_analysis::normalize_path` via `crate::sessions`.
- [x] **D6 — extraction-fixture harness → `normalize-facts`** (judgment call): ✅ DONE 2026-07-02.
  Moved the golden-diff engine (discovery + `run_case` + `expected.json` schema) into
  `crates/normalize-facts/src/extraction_fixtures.rs` (`discover_cases`/`run_case`,
  `FixtureCase`/`FixtureCaseResult`). `service/facts.rs::test_fixtures` `#[cli]` is now a thin
  wrapper mapping `FixtureCaseResult` → its `ExtractionFixtureCaseResult` report; the report
  structs + `OutputFormatter` stayed in main. ~380 LOC left the service layer.
- [x] **Rename `normalize-analyze` → `normalize-rank`**: ✅ DONE 2026-07-02. Crate dir,
  package name, workspace member, the single dependent (`normalize`) dep line, all
  `normalize_analyze::` Rust paths, SUMMARYs and living docs (cli-design,
  architecture-decisions, analyze-consolidation, POLISH) renamed in one commit. No
  `package =` aliasing existed. Crate count unchanged (44). User-facing → CHANGELOG.

### Command-surface decomposition roadmap (IN PROGRESS 2026-07-03)

**Supersedes the 2026-07-02 audit's "~62k of own code mostly legitimately stays" framing.**
That audit answered the *algorithm-extraction* question (algorithms already in crates —
true) but missed CLAUDE.md's second rule: *"a crate that owns a subcommand includes its own
`#[cli]` service, report structs, and `OutputFormatter` impls; the main crate just mounts
them."* Under that lens the command **surface** is substantially migratable.

**This IS the CLI taxonomy inversion, reached from the size direction.** Do not redesign —
the authoritative target (verb names + per-command owner + B0–B12 batch order) is
`docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md` (FINAL SCOPE). This roadmap
reconciles with it; see the [CLI command-taxonomy FULL INVERSION](#cli-command-taxonomy-full-inversion--seam-corrected-final-scope-high-priority)
section above for the batch plan. Reconciled/full audit with corrections:
**`docs/audit-2026-07-03-command-surface-decomposition.md`.**

**Open forks blocking execution (resolve before the batch that touches each):**
- **Metrics bucket A1 vs A2** (~19 subcommands, most of `rank` + `analyze docs`): A1 = keep
  `rank`/`trend` permanently main-resident (inversion-plan seam-eval *recommends A1*); A2 =
  extract a `normalize-metrics`-family crate + `metrics` verb. **UNRESOLVED** (the load-bearing
  call).
- **Dataflow trio home** (`liveness`/`effects`/`exceptions`): **RESOLVED 2026-07-03.** Code home
  FORCED to `normalize-facts` — the three commands read the `cfg_*` tables via `idx.connection()`;
  homing them in `normalize-cfg` is impossible (`facts → cfg` already exists, so `cfg → facts`
  would be a compile cycle, plus it contaminates pure in-memory cfg). Verb = `structure` (B5).
  This settles the earlier "the two docs disagreed (cfg vs facts)" note — the 07-03 roadmap's
  `normalize-cfg` suggestion is superseded by the dependency-cycle finding; facts/`structure` is
  final. **B5-execution alternative (parked, do not decide now):** if `structure liveness` naming
  grates, move `normalize-cfg`'s render `CfgService` into facts too and host a `cfg` verb there
  (yields `cfg liveness`), making `normalize-cfg` a pure library — allowed by the dep graph, costs
  a render-move + un-mounting cfg's service. Reconsider at B5-execution.
- **`search` verb collision** (RESOLVED 2026-07-03): inversion-plan B7 wires normalize-semantic
  as `search`, which clashes with the existing `search`→`grep` alias. **Decision:** drop the
  `search`→`grep` alias; `search` becomes the semantic verb. Alias removal executed at B7,
  atomically with mounting the verb (not now — nothing replaces it until B7). `find`→`grep` stays.
- **`analyze security`**: no compute crate in either map — genuinely unassigned (future
  security crate, or stays main). OPEN, non-blocking.
- **`coupling-clusters` → history** (RESOLVED): it is git-temporal (`co_change_edges`),
  belongs with `normalize-git-history` (B8/B9), NOT code-similarity. Corrected in the map below.

**Headline:** main is ~84k; ~21k is forced-to-stay vendored CLIs (publish trilemma); of the
~62k own code ~50k is migratable in principle. Realistic floor ≈ **30–34k** total → own
irreducible core ≈ **9–13k**. The mount pattern is already in production for 5 sub-services
(`normalize-budget`/`-cfg`/`-ratchet`/`-rules`/`-knowledge-graph`), each a `#[cli]` service +
reports + `OutputFormatter` with zero back-refs to main.

**Key facts:**
- **`OutputFormatter` is NOT a blocker** — it lives in `normalize-output`
  (`crates/normalize-output/src/lib.rs:94`); `crates/normalize/src/output.rs` is just
  `pub use normalize_output::*`. Feature crates impl it standalone today.
- **The real blocker is ORDERING** — the movable per-feature `service/*.rs` methods delegate
  into the in-main `crate::commands` module (171 refs). Service extraction is *downstream* of
  `commands/` extraction: move the `commands/<feature>/` impl into the crate first, then the
  thin `#[cli]` service method follows.
- **Two enablers** unblock the analyze family: (1) ✅ **DONE 2026-07-03** — index acquisition
  hoisted into the new `normalize-index` leaf crate (`open`/`ensure_ready`/`require_import_graph`,
  `IndexConfig`, `build_import_graph`/`ImportGraph`). Acquisition takes config **slices**
  (`&IndexConfig`+`&WalkConfig`), NOT `NormalizeConfig`, so feature crates depend on the leaf.
  Also broke the `graph ↔ architecture` cycle (build_import_graph now in the shared leaf) and
  fixed the standalone-`structure` `NORMALIZE_INDEX_DIR` bug. Main `crate::index`/`crate::paths`
  are thin wrappers/re-exports (main src −180 LOC); (2) `crate::config::NormalizeConfig`
  per-subcommand excludes — pass each crate its config slice (still pending).

**Migration map** (target → ~LOC → owner → notes):

| Target | ~LOC | Owner | Notes |
|---|---|---|---|
| Sessions (`commands/sessions/` + `service/sessions.rs`) | ~8k | NEW `normalize-sessions` | ✅ **DONE 2026-07-03** — extracted; main src 83,243 → 75,157 LOC (−8,086) |
| duplicates/duplicate-types/fragments | ~3k | `normalize-code-similarity` | `similarity` verb (B4). **Corrected:** clusters/coupling_clusters removed (git-temporal). |
| architecture/layering/depth_map | ~0.9k | `normalize-architecture` | `architecture` verb (B3). |
| graph/dependents/import-path | ~1.2k | `normalize-graph` | **Corrected:** these are `view` subcommands today (carve out of `view`, not analyze/rank). `graph` verb (B2). |
| clusters/coupling_clusters (git co-change) | ~0.5k | `normalize-git-history` (B8/B9) | **Corrected:** git-temporal (`co_change_edges`), NOT code-similarity. |
| liveness/effects/exceptions (dataflow) | ~0.9k | **RESOLVED → `normalize-facts`, `structure` verb (B5)** | Code home FORCED to facts (read `cfg_*` tables; `normalize-cfg` = `facts ⇄ cfg` cycle). cfg-consolidation alt parked for B5-exec. |
| provenance | ~0.75k | chat-sessions / session-analysis | — |
| small wrappers (generate/context/package/find_references) | ~2k | typegen/-context/-ecosystems/-scope | budget template |
| rank-metrics (hotspots, contributors, ownership, density, ceremony, test_ratio, call_complexity, size, docs, coupling, imports, uniqueness, module_health, surface, complexity/length/test_gaps, budget-metric) | ~5.7k | **NO owner — DECISION NEEDED** (`normalize-metrics` vs stay) | not blocked by OutputFormatter; blocked by absence of a home |
| view (`commands/view/` + tree/skeleton/parsers) | ~3.5k | none — intrinsically main | STAYS unless tree/skeleton/parsers extracted first |
| `service/edit.rs`, `service/facts.rs` | large | edit/facts crates | coupled to `crate::index`/shadow; later |
| aggregators, multi-repo, trend, init/update/sync, daemon, service composition | ~irreducible | — | genuinely stays |

**Ordered plan:**
- [x] **1. Sessions first** (~8k, no blockers) → new `normalize-sessions` (deps
  `normalize-chat-sessions` + `normalize-session-analysis`). ✅ **DONE 2026-07-03.** Coupling
  matched the audit exactly: only `crate::output` re-export, `super::` internals, `crate::sessions`
  re-export, and `resolve_pretty`. Handled by re-creating `crate::output`/`crate::sessions`
  re-export modules inside the new crate (so submodule files needed zero edits) and a local
  `resolve_pretty` that reads only the `[pretty]` config section (no dependency on main's
  `NormalizeConfig`). `sessions-web`/`axum` carried across as a crate feature. Main src 83,243
  → 75,157 LOC (−8,086); main mounts `normalize_sessions::service::SessionsService` in one line.
  Proves the full surface migration (commands subtree + `service/*.rs` method) end to end.
- [ ] **2. Build the two enablers** — (a) ✅ **DONE 2026-07-03**: shareable index acquisition
  hoisted into new `normalize-index` crate (slice-based config API, no `NormalizeConfig` dep;
  `build_import_graph` relocated, breaking `graph ↔ architecture`); (b) config excludes-slice
  still pending. Unblocks B2 (`graph`/`view` carve) and B3 (`architecture`).
- [ ] **3. Analyze families → existing owners** (~7.75k): code-similarity (B4), architecture
  (B3), graph (B2, out of `view`), dataflow trio (cfg *or* facts/structure — open fork B5),
  chat-sessions/session-analysis. `search` (B7): drop `search`→`grep` alias atomically with
  mounting the semantic verb (collision resolved 2026-07-03).
- [ ] **4. DECISION on the ~5.7k rank-metrics** — designate `normalize-metrics` as owner vs.
  leave in main. The one genuinely open architectural call here.
- [ ] **5. Small wrappers** (~2k) — generate/context/package/find_references, budget template.

### Language trait: remaining .scm migration

**Known locals.scm scope engine limitation:**
- Nested destructuring (e.g. `{ a: { b } }` in parameters) requires recursive queries which
  tree-sitter does not support. One level of object/array destructuring IS covered for JS/TS/TSX.
  Fixing deeper nesting would require engine-level recursion (walk into nested patterns).

### Language implementation depth

- [x] Audit (2026-03-12): 47/84 languages at 100% .scm coverage. Full gap list below.
- [x] Decoration tests (2026-04-27): All 45 `decorations.scm` tests upgraded from smoke tests to `assert_decorations_contains` with expected fragments. Fixed `lean.decorations.scm` (`(attributes)` → `(attribute)`). Removed `; NOTE: verify node type` comments from gleam and lean queries. Added `///` doc comment to zig fixture, `[[nodiscard]]` to cpp fixture, `|||` doc comment to idris fixture. CI enforcement via `NORMALIZE_REQUIRE_GRAMMARS=1` env var: decoration tests panic instead of silently skip when the env var is set but grammars are absent — prevents the "310 passed with zero assertions" false-positive under `cargo test -q`.
- [x] Decoration node type audit (2026-04-27): Investigated all unverified/incorrect node types across 8 decoration query files. C: `preproc_call` is correct for `#pragma` etc. — `#include` is `preproc_include` (not a decoration); test now asserts on `/* ... */` comment instead. ObjC: fixed `preproc_call` → `preproc_include` (`#import` aliases into `preproc_include` in the ObjC grammar). Ada: `pragma_g` confirmed correct per RM 2.8 in grammar; removed NOTE comment. Idris: removed erroneous `(doc_comment)` and `(pragma)` — `|||` is parsed as `(comment)`, pragmas are specific nodes (`pragma_inline` etc.) with no generic wrapper. Julia: fixed `macro_expression` → `macrocall_expression` (verified in grammar); added `@inline function classify` to fixture. Perl: fixed `pod_statement` → `pod` (verified in grammar); added POD block to fixture. Clojure: added `(meta_lit)` for reader metadata (`^:deprecated` etc.); added example to fixture. Verilog: added `(attribute_instance)` for `(* ... *)` attributes; added example to fixture. Also fixed pre-existing clippy `items_after_test_module` lint in `rust.rs`.
- [x] **Idris `|||` doc comments** (2026-04-27): Implemented `#match?`/`#not-match?`/`#eq?`/`#not-eq?` predicate evaluation in `normalize_languages::satisfies_predicates` (`query_predicates.rs`). Wired into `collect_captures` in `query_fixtures.rs` and `decoration_extended_start` in `normalize-refactor/src/actions.rs`. `idris.decorations.scm` correctly captures all `(comment)` nodes — adjacency filtering in `decoration_extended_start` already ensures only immediately-preceding comments move with a symbol, so `#match?` filtering on `|||` is not required for correctness. Unknown predicates pass (future-proof).

**Feasible gaps (grammar supports it, .scm not written):**

High-value:
- [x] TSX imports.scm (reuse TypeScript logic)
- [x] ~~Svelte imports.scm~~ — grammar produces opaque import_statement nodes; extraction handled by Rust extract_imports() + embedded JS injection
- [x] ~~Vue imports.scm~~ — grammar doesn't parse JS content; extraction depends entirely on embedded JS injection
- [x] ~~GraphQL imports.scm~~ — genuinely no import syntax in grammar (federation directives are just regular directives)
- [x] ~~SQL imports.scm~~ — genuinely no import syntax modeled in grammar (IMPORT FOREIGN SCHEMA not in tree-sitter-sql)
- [x] Jinja2 calls.scm (function/method/filter/test/call-statement)
- [x] Thrift tags.scm (struct/union/exception/enum/service/function/typedef/const)
- [x] Dockerfile tags.scm (FROM...AS stages, ARG, ENV)

80% languages (types.scm assessed — only Typst feasible):
- [x] Typst types.scm (parameter type annotations via `tagged` nodes in `let` bindings)
- [x] ~~SCSS types.scm~~ — no type system; grammar has no type-like nodes
- [x] ~~Perl types.scm~~ — no static type annotations in grammar (Perl is dynamically typed; type constraints via Moose/Type::Tiny are runtime, not in CST)
- [x] ~~Prolog types.scm~~ — no type system in grammar (Prolog is untyped; type annotations via library predicates are just regular terms)
- [x] ~~AWK types.scm~~ — no type system; all values are strings/numbers contextually
- [x] ~~Fish types.scm~~ — shell language, no type annotations in grammar
- [x] ~~Zsh types.scm~~ — shell language, no type annotations in grammar
- [x] ~~Vim types.scm~~ — VimScript has no type annotations in grammar
- [x] ~~Jq types.scm~~ — no type system; jq operates on JSON values dynamically
- [x] ~~Meson types.scm~~ — build system DSL, no type annotations in grammar
- [x] ~~CMake types.scm~~ — build system scripting, no type annotations in grammar

Config/markup:
- [x] Nginx calls.scm (simple_directive and block_directive name captures)
- [x] Caddy tags.scm (site blocks, snippets, named matchers, handle/route directives)

**Genuinely unsupported (correct as None/empty):**
- Bash types (no type system)
- JSON/YAML/TOML/XML/Markdown imports/calls/complexity (data formats)
- HTML/CSS calls/complexity/types (markup/style)

### Comprehensive language fixtures (long-term, nix flake verification)

- [x] Design fixture schema: input source file → expected symbols, imports, calls (2026-05-06):
      `crates/normalize-languages/tests/fixtures/<lang>/<case>/input.<ext>` + `expected.json`.
      Schema: `{exhaustive, symbols: [{name, kind, line}], imports: [{module, name, line}], calls: [{callee, line}]}`.
      All fields optional; subset matching by default; `"exhaustive": true` for full-list checking.
- [x] Fixture runner: `normalize structure test-fixtures [--lang <lang>] [--fixture-dir <dir>] [--update]`
      (2026-05-06): language-agnostic runner in `crates/normalize/src/service/facts.rs`; discovers
      `<lang>/<case>/` subdirectories, extracts via `SymbolParser`, diffs against `expected.json`.
      `--update` writes actual output as new expected (bootstrap mode). Report: `ExtractionFixtureTestReport`.
- [x] Seed fixtures for 3 languages (2026-05-06): `rust/basic-function/`, `python/imports/`, `typescript/classes/` — all passing.
- [ ] Nix flake approach: each language's fixtures run in a devShell with the real compiler/runtime
      available — lets us verify against `rustc`, `tsc`, `python`, `go build` etc. for ground truth
- [ ] Seed fixtures for top 20 languages (high confidence, hand-verified) — long-term
- [ ] Automated fixture generation: use `normalize analyze` + LLM to bootstrap expected outputs,
      then human-verify before committing
- [ ] CI integration: `nix flake check` runs all language fixture suites in parallel

### ~~Qualified/namespaced import resolution in the facts index~~ (done 2026-03-12)

`calls` table now has `callee_resolved_file TEXT` column (schema v6). After import resolution,
`resolve_all_calls()` populates it by joining calls with imports and same-file symbol defs.
`find_callers()` uses `callee_resolved_file` for precise disambiguation; falls back to
import-based matching when NULL (external/unresolved modules).
- [x] Store caller/callee with module qualification in facts index (`callee_resolved_file` column)
- [x] Resolve calls after imports in both full and incremental refresh paths
- [x] `find_callers` uses resolved file — `edit rename`, `call-graph`, LSP references all benefit

### Remaining work (short items)

- Namespace-qualified lookups: `normalize view std::vector`, `normalize view com.example.Foo`
  - Requires language-specific namespace semantics - low priority
- Shadow worktree: true shadow-first mode (edit in shadow, then apply)
  - Current: --shadow flag works, but not default for all edits
  - Zero user interruption (user can edit while agent tests in background)

### Config parse failures are silent (P0 bug)

`load_rules_config` silently returns defaults when config.toml fails to parse (e.g.
duplicate TOML key). This means a typo in config silently disables ALL rule overrides,
severity settings, and allow patterns — with no warning. Users see unexpected rule
behavior and have no way to know the config isn't loading.

Fixes:
- [x] **Warn on parse failure** — `load_rules_config` prints the parse error to stderr
  when config.toml exists but fails to deserialize. Falling back to defaults is OK as
  long as the user sees the warning.
- [x] **`normalize config validate`** — deep validation of config.toml: TOML syntax (duplicate
  keys), JSON Schema compliance, serde deserialization, and rules config parsing. Checks both
  project and global config. Exits non-zero on errors for CI/hook use.
- [x] **Validate on `rules run`** — `load_rules_config` already emits `eprintln!` warnings when config parse fails (added in the config validation work); warning appears in CI output via stderr.

### Configuration system

Sections: `[daemon]`, `[index]`, `[aliases]`, `[view]`, `[analyze]`, `[grep]`, `[pretty]`, `[serve]`

Adding a new section (3 places):
1. Define `XxxConfig` struct with `#[derive(Merge)]` + `XxxArgs` with `#[derive(Args)]` in command module
2. Add field to NormalizeConfig
3. Add `run(args, json)` function that loads config and merges

Candidates: `[workflow]` (directory, auto-run)

### Schema-Driven Config UI — remaining follow-ups

- `normalize rules validate` intentionally separate: rule-ID validation (checks against live registry) can't be expressed in JSON Schema. Not redundant with `config validate` — they check different things.
- Extract engine into `normalize-config-ui` crate for reuse / publication (stretch goal)
- Propose `#[config]` proc macro to server-less (stretch goal — superseded by `#[derive(Config)]`;
  filed nested struct support + merge semantics requests in server-less TODO.md 2026-03-10)

### Retire `--schema` hack in `main.rs`

`handle_schema_flag()` in `crates/normalize/src/main.rs` reads raw `std::env::args()` before
clap and prints `schemars::schema_for!(NormalizeConfig)`. It is stranded legacy from before the
server-less `#[cli]` migration.

The sanctioned replacement exists: `NormalizeConfig` already derives `server_less::Config`, and
server-less `#[program(config = T)]` generates a `config schema` subcommand automatically.

**Cleanup steps:**
1. Wire `config = NormalizeConfig` into the `#[cli]`/`#[program]` attribute on `NormalizeService`.
2. Delete `handle_schema_flag()` and its call site.

**Caveat before deleting:** The current `--schema` output is an envelope
`{ config_path, format, schema }` consumed by Nursery integration (introduced in moss commit
d19654c9, "Nursery integration"). The server-less `config schema` subcommand emits raw JSON
Schema. Reconcile the envelope shape with the Nursery consumer before removing the hack — don't
break Nursery silently.

**Why this matters:** The hack squats the root `--schema` name and blurs two distinct concepts
(config-file schema vs CLI-tree schema), blocking the naming space needed for the
server-less whole-tree "manual" surface.

**Dependency satisfied (2026-06-19):** that whole-tree surface has now shipped as `--manual` in
server-less 0.5.0, so the blocking dependency is gone — the retirement is now actionable once the
0.5.0 bump lands (see "server-less 0.5.0 adoption" below). Still honor the Nursery envelope caveat
above before deleting `handle_schema_flag()`.

### ~~Complexity Hotspots~~ (resolved - max now 22)

All original hotspots resolved. Remaining max is `split_query_patterns` (22) in runner.rs.

- [x] `crates/normalize/src/commands/analyze/query.rs:cmd_query` (58→15) — already resolved
- [x] `crates/normalize/src/commands/daemon.rs:cmd_daemon` (54→1) — already resolved
- [x] `crates/normalize-syntax-rules/src/runner.rs:run_rules` (53→18)
- [x] `crates/normalize-syntax-rules/src/runner.rs:evaluate_predicates` (53→11)
- [x] `crates/normalize/src/commands/analyze/mod.rs:run` (51→5) — already resolved
- [x] `crates/normalize/src/commands/tools/lint.rs:cmd_lint_run` (48→15) — already resolved
- [x] `crates/normalize/src/tree.rs:collect_highlight_spans` (42→9)
- [x] `crates/normalize/src/tree.rs:capture_name_to_highlight_kind` (23→2)
- [x] `crates/normalize/src/tree.rs:render_highlighted` (23→8)
- [x] `crates/normalize/src/tree.rs:docstring_style_for_grammar` (21→5)

### CLI Internal Consolidation

**Top-level command level issues (low priority):**
- [x] ~~`context` could be `normalize view context [path]`~~ — redesigned as a standalone frontmatter-filtered system (`docs/context-redesign.md`); no longer relevant to merge with view.
- [x] ~~`normalize context` v2 follow-ups: daemon caching (v2 in design doc), embedding search (v3).~~ — v2 (daemon caching) shipped; v3 (`--semantic` embedding search) shipped: `normalize context --semantic "query"` returns top-k context blocks by cosine similarity. Context blocks embedded via `source_type='context'` during `structure rebuild`. Hybrid `--semantic --match` supported.
- [x] ~~`normalize context` migration helper for old `.context.md` files~~ — `normalize context migrate` (dry-run by default, `--apply` to perform); deprecation warning added to `get_merged_context` for `view --dir-context`.
- `aliases` is a cross-cutting utility. Too small for top-level but has no clear parent. Low priority.

### `normalize init --setup` extensions

**Remaining:** Review default-enabled rule set. Several rules generate high noise; guided
setup is the cure. Also: `normalize init --setup` currently only covers rules — extend to
other project-level decisions as they emerge (e.g., exclude patterns, SUMMARY.md enforcement).

**Default-enabled inconsistencies (2026-03-13 audit):**
- ~~Debug-print rules: Go (`fmt-print`) and Python (`print-debug`) enabled by default, but C/C++/Java/Kotlin/PHP/Rust/Swift/C# equivalents all disabled. Should be consistent.~~ Resolved: all debug-print rules were already consistently disabled by the frictionless OOTB change; Go/Python/JS only enabled via local config override for dogfooding.
- ~~Correctness rules that should be enabled by default: `go/defer-in-loop`, `go/sync-mutex-copied`, `swift/force-unwrap`, `python/raise-without-from`, `python/use-with`, `ruby/method-missing`.~~ Done: all 6 enabled by default with `recommended = true`.
- ~~Potentially too aggressive defaults: `rust/chained-if-let` (error severity for style).~~ Done: downgraded to warning. `rust/numeric-type-annotation` already disabled. Tuple-return rules already info-level and disabled.

**Wizard UX improvements:**
- [x] Show rules with zero violations (summary count + pointer to `rules list`)
- [x] `recommended = true` frontmatter for genuine bug/correctness rules vs style opinions
- [x] Recommended rules shown first in wizard (sorted before violation count)
- [x] Standalone `normalize rules setup` command (don't require re-running `init`)
- [x] Group by tag/category instead of flat violation-count sort
- [x] Add batch operations: "enable all [correctness] rules", "disable all [style] rules"
- [x] Show practical impact: "2 violations (quick fix)" vs "847 violations (major cleanup)"

### SARIF engine actionable output

- ~~`rules run --engine sarif` could show which SARIF tools had errors (not done)~~ Done: tool errors captured in `DiagnosticsReport.tool_errors` and shown in text/pretty output

### Duplicate/clone detection improvements

- [x] Per-subcommand excludes in config: `[analyze.duplicates] exclude = [...]` via `#[serde(flatten)]` HashMap on `AnalyzeConfig`. Wired into all analyze subcommands that accept `--exclude`: duplicates, complexity, length, docs, health, all, test-gaps, uniqueness, hotspots, files, size, coupling, coupling-clusters, ownership, fragments, skeleton-diff.
- [x] "Parallel impl directory" heuristic: if >=5 pairs originate from the same directory pair, fold them into a suppressed note (e.g., "388 pairs suppressed across 10 directory groups"). Applied to exact-functions, similar-functions, and similar-blocks when `!include_trait_impls`. Handles 2-location pairs and multi-location groups (up to 2 distinct directories).
- [x] `similar-functions`: body-pattern cluster suppression — connected-component analysis on pair graph; components spanning 3+ files with 5+ pairs are suppressed as `SuppressedBodyPatternGroup`. Catches Language trait impl case (e.g. `extract_imports` across 20 structs). Applied when `!include_trait_impls`. `suppress_widespread_body_patterns()` in `duplicates.rs`.
- ~~Consider min-lines bump for `similar-blocks` (currently 10)~~ **Done**: bumped default to 15 for similar-blocks, made configurable via `--min-lines` CLI flag and `[analyze.duplicates] min_lines` in config

### Syntax Ruleset Breadth

- **Trigger for fix infrastructure**: once enough rules have structural auto-fixes that need correct indentation, build the corpus-based indentation model (see `docs/prior-art.md` § "Corpus-based indentation model"). Don't build it speculatively.
- **tree-sitter-go note**: `block` → `statement_list` → statements. Queries must use `statement_list` as intermediate node; `(block (return_statement))` won't match.
- Phase 3b builtin rules: more builtin rules, sharing improvements (see `docs/design/builtin-rules.md`)
  - [x] Java rules (6): `system-print`, `empty-catch`, `print-stack-trace`, `magic-number`, `suppress-warnings`, `thread-sleep`
  - [x] C/C++ rules (4): `c/printf-debug`, `c/goto`, `c/magic-number`, `cpp/cout-debug`
  - [x] C# rules (6): `console-write`, `empty-catch`, `goto`, `magic-number`, `thread-sleep`, `suppress-warnings`
  - [x] Kotlin rules (5): `println-debug`, `empty-catch`, `magic-number`, `thread-sleep`, `suppress-warnings`
  - [x] Swift rules (5): `print-debug`, `empty-catch`, `magic-number`, `force-unwrap`, `thread-sleep`
  - [x] PHP rules (5): `debug-print`, `empty-catch`, `goto`, `magic-number`, `eval`
  - Semantic rules system: for rules needing cross-file analysis (import cycles, unused exports, type mismatches). Current syntax-based rules are single-file AST queries; semantic rules need index-backed analysis. Separate infrastructure, triggered differently (post-index vs per-file).

### ~~Rule tags system~~ (done)

- [x] Deterministic tag color hashing in `--pretty` output (FNV-1a hash, 10-color curated palette at OKLCH L≈0.65, red/yellow reserved for severity)

### ~~normalize-ratchet: metric regression tracking~~ (done 2026-03-22)

- [x] `normalize-metrics` crate: shared `Metric` trait, `MetricFactory`, `Aggregate` enum + `aggregate()`, `filter_by_prefix()` — depended on by both ratchet and budget
- [x] `normalize-ratchet` crate with 6 metrics: complexity, call-complexity, line-count, function-count, class-count, comment-line-count
- [x] 6 CLI commands (behind `cli` feature): `measure`, `add`, `check`, `update`, `show`, `remove`
- [x] Entries are `(path, metric, aggregation) → value`; path can be dir/file/symbol (`file/Parent/fn`); always aggregated — symbol path is degenerate case (n=1)
- [x] Baseline stored in `.normalize/ratchet.json`; 6 aggregation strategies (mean/median/max/min/sum/count); defaults configurable via `[ratchet]` / `[ratchet.metrics.<name>]` in `.normalize/config.toml`
- [x] `MetricFactory` type alias outside `cli` feature; `RatchetConfig` wired into `NormalizeConfig` via `#[param(nested, serde)]`
- [x] Native rules integration: `normalize rules run` detects regressions via `ratchet/<metric>` rule IDs
- [x] `--base <git-ref>` on `check` and `measure` for historical comparison — originally via git worktrees; migrated to gix blob reads + tempdir (`refactor(ratchet): replace git worktrees with in-memory gix blob reads`)
- [x] `normalize-budget` crate: diff-based budget system; each entry has `(path, metric, aggregate, ref) → {max_added, max_removed, max_total, max_net}` (all optional); budget stored in `.normalize/budget.json`
- [x] 7 diff metrics: lines, functions, classes, modules, todos, complexity-delta, dependencies
- [x] Native rules integration: `budget/<metric>` rule IDs alongside ratchet rules

**Follow-up ideas (not planned):**
- `--base` now uses gix blob reads + tempdir (no git binary); caching measurements per git-ref in `.normalize/ratchet-cache/` could further speed up large repos
- Call-graph BFS is intra-project only (no cross-crate edges); future: integrate with `normalize-graph` if cross-crate call data exists
- Trend charts (`normalize ratchet trend`) could visualize metric history over git log

### ~~CI readiness~~ (done — 0.2.0 shipped)

- [x] `normalize ci` command — `--no-syntax`/`--no-native`/`--no-fact`/`--strict`/`--sarif` flags, structured output, non-zero exit on errors.
- [x] Install script — `install.sh` + `install.ps1`, platform/arch detection, SHA256 verification, version pinning via `NORMALIZE_VERSION`.
- [x] CI documentation — `docs/ci.md` with GitHub Actions/GitLab/CircleCI snippets.
- [x] Version bump to 0.2.0 — all 38 published crates bumped; `normalize update` works against GitHub releases.
- [x] Polish pass — `--help` audit, exit codes verified, smoke-tested on external repos.

### Tighten threshold rules to zero violations

Rules enabled at generous thresholds (long-file 6400, high-complexity 65, long-function 350)
to establish a floor. Target: reduce all to reasonable thresholds (long-file 500,
high-complexity 20, long-function 100) by splitting/refactoring violating code. Track via
ratchet once integrated.

### Claude Code hooks for lint-on-save

Once Pillar 7 (sub-100ms hot path) delivers acceptable perf, add Claude Code hooks that
run `normalize rules run --files <changed>` after every tool call. This gives agents
immediate feedback on violations they introduce. Blocked on Pillar 7.

### ~~Phase out *-allow files~~ ✓ Done

All 7 legacy allow files migrated to `config.toml` and file-loading code removed:
- `large-files-allow` → `[rules.rule."long-file"] allow = [...]`; `LongFileRule::new()` now takes allow list as parameter
- `hotspots-allow` → `[analyze] hotspots_exclude = [...]`
- `duplicate-blocks-allow` → `[analyze.duplicate-blocks] allow = [...]`
- `duplicate-functions-allow` → `[analyze.duplicate-functions] allow = [...]`
- `duplicate-types-allow` → `[analyze.duplicate-types] allow = [...]`
- `similar-blocks-allow` → `[analyze.similar-blocks] allow = [...]`
- `similar-functions-allow` → `[analyze.similar-functions] allow = [...]`
`SubcommandConfig` gained `allow: Vec<String>` field; `AnalyzeConfig::allows_for()` reads it.

---

## P2 — Structural Improvements / Larger Refactors

### Rules Unification — remaining threads

4. [x] **Unify rule engine config** — done: all four engines (syntax, fact, native, SARIF) consume the shared `RulesConfig` from `normalize-rules-config`. `RuleOverride` supports severity/enabled/allow/tags/filenames/paths. `global_allow` applied consistently.

5. [x] **SARIF passthrough engine** (`--engine sarif`) — implemented: `SarifTool` config type in `normalize-rules-config`, `run_sarif_tools()` in runner, `[[rules.sarif-tools]]` in config.toml. Runs with both `--type sarif` and `--type all` (default).

6. **Health findings → native rules** — Phase 1 done: `long-file`, `high-complexity`, `long-function` native rules added to `normalize-native-rules` with default thresholds (500 lines, complexity 20, 100 lines). All default disabled (advisory). `--rule <id>` implicitly enables. `NativeRuleDescriptor` gained `default_enabled` field. Follow-ups: configurable thresholds via `RuleOverride` (needs numeric threshold field), `analyze health` aggregation of rule diagnostics.

### Incremental-first architecture

The current architecture is batch-oriented: commands scan the whole workspace, produce a report, and exit. This works for CLI but is wrong for LSP and other interactive consumers. The goal is to make incrementality a first-class concern throughout the stack.

**What's done:**
- [x] `FileIndex::update_file()` — single-file re-index without full rebuild
- [x] Per-file syntax rule evaluation in LSP (run rules only on saved file)
- [x] Two-tier LSP diagnostics: immediate syntax, debounced fact rules
- [x] Daemon calls `incremental_call_graph_refresh()` after detecting changes
- [x] SQLite findings cache for native + syntax rules (replaces JSON; per-file mtime-keyed)
- [x] `FileRule` trait — new native rules get caching/parallelization/file-walking automatically
- [x] Incremental git walk for stale/missing-summary (walk only new commits, not full history)
- [x] Batched uncommitted-changes check (one gix status walk, not per-directory)
- [x] Daemon fire-and-forget spawn (no 2s socket wait blocking every command)
- [x] `rules run` wired to try daemon cache before local computation (`try_rules_via_daemon()`)

**Remaining:**
- **Daemon nested-runtime panic**: `daemon run` creates a tokio runtime inside `#[tokio::main]` — panics with "Cannot start a runtime from within a runtime". Must fix before daemon path is usable.
  - Note 2026-04-27: the daemon code now uses `Handle::current()` (no nested runtime). This may already be fixed — needs verification.
- [x] **Parallel fact rule evaluation** (2026-04-27): `run_rules_batch` now runs each rule on a separate rayon thread. ~2.2× wall-time speedup (5.5s → 2.5s on this codebase). JIT disabled in parallel path: `jit_recent_indices` panics for sink relations when two JIT engines initialize concurrently — upstream ascent-interpreter bug, see next item.
- **JIT threading bug in ascent-interpreter** (2026-04-27): `engine.run()` with JIT enabled panics with `index out of bounds: len is 0, index is 0` in `jit_stratum_advance_s4_inner` (packed_helpers.rs line 872) when two JIT engines run concurrently. Root cause: `jit_recent_indices` Vec is empty for `jit_is_sink = true` relations; the concurrent access exposes an otherwise latent issue. Fix in ascent-interpreter needed before JIT can be re-enabled in the parallel `run_rules_batch`.
- Syntax rules load and compile all tree-sitter queries on every invocation
- **Fact rules**: incremental Datalog. When facts for one file change, re-derive only affected conclusions. This is hard — may need semi-naive evaluation with change tracking.
- [x] **Watch mode**: `normalize watch` that keeps the index live and re-runs checks on file changes (inotify/fsevents). The LSP server is one consumer; a TUI dashboard could be another. — done: `normalize daemon watch` streams file-change events to terminal; see L91 `[x] normalize watch CLI`.
- **`SymbolIndex` trait**: injected API for symbol resolution (daemon → index → parse-on-miss). See `docs/design/daemon-as-kernel.md`.

**Next incremental steps:**
1. Fix JIT threading in ascent-interpreter (then re-enable JIT in `run_rules_batch` parallel path for ~4× speedup vs current interpreted parallel)
2. Verify daemon nested-runtime panic is gone (use `Handle::current()` — may already be fixed)
3. Persistent `GrammarLoader` in LSP (don't re-create `SkeletonExtractor` per request)
4. File-level dependency tracking for diagnostic invalidation
5. `SymbolIndex` trait — wire view/edit/analyze through injected API
6. Incremental fact rule evaluation (long-term, research needed)

### Lint / Analysis Architecture

See `docs/lint-architecture.md` for full design discussion.

**Architecture decision: Datalog for semantic queries**
- Datalog is the standard for code analysis (CodeQL, Semmle, codeQuest)
- Recursion essential for code queries (transitive deps, call graphs)
- Safe Datalog: guaranteed termination, right level of expressiveness

**Implementation plan:**
- [x] ~~All rules (builtin + user) compile to dylibs via Ascent + `abi_stable`~~ — abandoned: dylib approach caused heap corruption (`corrupted double-linked list`) from `RString/RVec` allocator mismatch across dylib boundary. Replaced with interpreted `.dl` files via `normalize-facts-rules-interpret` (no dylib loading at all).
- [x] Same infrastructure for both - builtins ship pre-compiled, users compile theirs (done via `.dl` files)
- [x] Same syntax for both (rules can graduate from user to builtin) — done: `.dl` files for all rules
- See "Facts & Rules Architecture" section below for full plan

**Rule tiers:**
1. `syntax-rules` (exists): AST patterns, no facts needed
2. `facts-rules` (new): Datalog over extracted facts (symbols, imports, calls)
3. `normalize-lint` (new): escape hatch for complex imperative logic

**Differentiation from CodeQL:**
- CodeQL: deep analysis (types, data flow, taint), ~12 languages
- normalize: structural/architectural analysis, ~98 languages
- Focus areas: circular deps, unused exports, module boundaries, import graph metrics

**Architectural analysis next iteration:**
- [x] Boundary violation rules (configurable: "services/ cannot import cli/") — implemented 2026-05-06:
      `boundary-violations` native rule in `normalize-native-rules/src/boundary_violations.rs`;
      config via `[rules.rule."boundary-violations"] boundaries = ["A/ cannot import B/"]`;
      default disabled, requires structural index; wired into daemon, CI service, and `normalize rules run`.
- [x] Re-export tracing (follow `pub use` to resolve more imports) — implemented 2026-05-06:
      `@import.reexport` capture in `rust.imports.scm`, `typescript.imports.scm`, `javascript.imports.scm`;
      `is_reexport` column in `imports` table; `trace_reexports()` in `FileIndex` runs after
      `resolve_all_imports()` to follow chains up to depth 10; schema bumped to 12.

Rules (custom enforcement, future):
- [x] Module boundary rules ("services/ cannot import cli/") — covered by `boundary-violations` native rule (see above)
- [x] Threshold rules ("fan-out > 20 is error") — covered by `high-fan-out` and `high-fan-in` native rules (index-based, configurable threshold, default disabled, tags: architecture/coupling)
- [x] Dependency path queries ("what's between A and B?") — `normalize view import-path <from> <to>` (BFS over resolved import graph; `--all` for all simple paths, `--reverse` to flip direction)

**Rule unit testing (`normalize rules test`):**
- [x] Inline marker format (fourslash-style): `normalize rules test <file>` runs all enabled syntax
      rules against a source file and asserts annotated lines (`// error[rule-id]`) match actual
      diagnostics. Language-agnostic; multiple annotations per line supported. Implemented in
      `crates/normalize-rules/src/service.rs`.
- [x] Fixture format alternative: `test.input.<ext>` + `test.expected.json` for cases where inline
      markers are awkward (multi-file rules, fact rules over a whole project).
      Implemented as `normalize rules test-fixtures` in `crates/normalize-rules/src/service.rs`.
      Auto-discovers cases under `.normalize/rule-tests/` (single-file: `<stem>.input.<ext>` +
      `<stem>.expected.json`; multi-file: `<case>/input/` + `<case>/expected.json`).
      `--update` flag rewrites `expected.json` with actual findings for bootstrap.
      `message` field uses substring matching. Example fixtures in `.normalize/rule-tests/`.

**Facts & Rules Architecture:**
- [x] `normalize rules compile <rules.dl>` — validates syntax + checks all relation names against declared/built-in set; exits 1 on errors; CI-friendly
- [x] ~~Self-install builtin dylib~~ — no longer applicable; builtins are embedded `.dl` files in `normalize-facts-rules-interpret/src/builtin_dl/`, no dylib or copy step needed.

### normalize-manifest: eval-backed parsing (`eval` feature gate)

Heuristic parsers in `normalize-manifest` cover ~95% of real-world files but fail on
code-as-config formats (Gemfile, mix.exs, build.gradle, flake.nix, Package.swift) where
variables and conditionals can't be resolved from text alone.

**Design**: feature-gate eval capability inside `normalize-manifest` itself (not a
separate crate, not in `normalize-local-deps`). Eval is about parsing fidelity, not
ecosystem discovery.

```rust
// always available — heuristic
pub fn parse_manifest(filename, content) -> Option<ParsedManifest>

// feature = "eval" — tries subprocess first, falls back to parse_manifest automatically
pub fn parse_manifest_eval(filename, content, root: &Path, policy: EvalPolicy) -> Option<ParsedManifest>
```

`parse_manifest_eval` degradation order:
1. Official dump command (runtime-native, safe) → perfect results
2. Wrapper script executed in the runtime → declared deps with variables resolved
3. `parse_manifest` heuristic fallback → always returns something

`EvalPolicy`: `IfAvailable` (try, fall back silently) | `Required` (error if runtime absent)

**Official dump commands** (safe, no arbitrary code exec):
- `cargo metadata --format-version 1` (Rust)
- `go list -json -m all` (Go)
- `npm ls --json` (Node)
- `swift package dump-package` (Swift — already outputs JSON)
- `bundle list --format json` (Ruby, Bundler ≥ 2.4)
- `mix deps.tree` (Elixir — needs shaping into ParsedManifest)

**Wrapper scripts** (executes project code — opt-in only):
- Gemfile: `ruby -r bundler -e 'puts Bundler.definition.dependencies.to_json'`
- mix.exs: elixir wrapper that loads Mix.Project and captures deps config
- flake.nix: `nix eval .#inputs --json`
- build.gradle: inject a task that dumps resolved configurations

**Tree-sitter middle tier** (no execution, better than heuristic):
- Worth considering for code-as-config formats as a tier 1.5 between heuristic and eval
- Handles multiline expressions, strips comments, correct block boundaries
- Still can't resolve runtime variables, but dramatically fewer false negatives
- Belongs in same feature gate or a separate `tree-sitter` feature in normalize-manifest

### Rust Redesign Candidates

- Rules engine: consider semgrep/ruff integration instead of custom
- Plugin system: Rust trait-based plugins or external tool orchestration

### Language Capability Traits — remaining

- [ ] Add `as_imports()`, `as_complexity()`, `as_edit()` capability queries — not ready yet. Sparsity check (2026-03-11, verified): `extract_imports` stub rate is ~1.4% (1 language: asm, with explicit comment). 72% have real impls; 7% use `.imports.scm` query files; 21% are config/data languages with no import concept (correct behavior). Far below 50% threshold — `as_imports()` trait is NOT warranted. Revisit after adding more languages.

### Tooling

- Read .git directly instead of spawning git commands where possible
  - Default branch detection, diff file listing, etc.
  - Trade-off: faster but more fragile (worktrees, packed refs, submodules)
- Documentation freshness: tooling to keep docs in sync with code
  - For normalize itself: keep docs/cli/*.md in sync with CLI behavior (lint? generate from --help?)
  - For user projects: detect stale docs in fresh projects (full normalize assistance) and legacy codebases (missing/outdated docs)
  - Consider boy scout rule: when touching code, improve nearby docs
- Semantic editing next steps:
  - **Structural pattern rewrite** — highest-value next direction. `syntax query` already
    matches ast-grep metavariable patterns; `edit` already does structural refactoring.
    What's missing is the bridge: match an AST pattern anywhere → rewrite to another
    pattern (comby / ast-grep `--rewrite`). Pattern-match exists, structural-edit exists,
    pattern→pattern rewrite doesn't. Surfaces the existing primitives into the headline
    capability of comby/ast-grep, with no new substrate required.
    Example: `--pattern 'fn $name($args) -> $ret { ... }' --rewrite '...'` AST-level, not regex
  - Integration with shadow git: checkpoint before large refactors, rollback on failure
  - **Local rename (`edit rename path/func/local new_name`)**: scoped rename within a block.
    No index needed. Two tiers:
    - Conservative: `replace_all_words` within the container's byte range, stop at any nested
      binding with the same name (avoids worst-case shadowing corruption, misses outer refs past inner shadow)
    - Correct: tree-sitter scope walk — find the declaration node, then walk identifier nodes
      that resolve to the same binding. Language-specific scope rules required (Rust, JS, Python differ).
    SkeletonExtractor doesn't surface locals; needs a dedicated local-variable locator.
- Cross-file refactors: `normalize move src/foo.rs/my_func src/bar.rs`
  - Move functions/types between files with import updates
  - Handles visibility changes (pub when crossing module boundaries)
  - Updates callers to use new path
- Structured config crate (`normalize-config`): trait-based view/edit for known config formats (TOML, JSON, YAML, INI). Unified interface across formats. (xkcd 927 risk acknowledged)
  - Examples: .editorconfig, prettierrc, prettierignore, oxlintrc.json[c], oxfmtrc.json[c], eslint.config.js, pom.xml
  - Open: do build scripts belong here? (conan, bazel, package.json, cmake) - maybe separate `normalize-build`
  - Open: linter vs formatter vs typechecker config - same trait or specialized?
  - Open: reconsider normalize config format choice (TOML vs YAML, JSON, KDL) - rationalize decision

### Candidate rule: drifted-dispatch-tables

See `docs/ad-hoc-dispatch.md` for the full design tenet and ecosystem evidence.

**Status:** Viable. Extraction sufficient (spike confirmed). Rule-side logic needed.

**What it detects:** N parallel dispatch tables over the same closed name-set where one registry/trait/visitor belongs — strongest signal is *drift* (set-difference of key-sets between parallel tables). Reproduced wick and marinada drift mechanically using `normalize syntax query`.

**Rule-side logic needed (extraction is not the bottleneck):**
1. Per-function scoping by row-range — to isolate individual dispatch tables rather than treating a file as one bag of keys.
2. Jaccard key-set clustering — to identify which tables draw from the same "roster" vs. unrelated dispatchers in the same file. Load-bearing: without it the signal is buried in false positives.
3. Set-difference + min-table-size threshold + baseline/allowlist — to distinguish genuine drift from intentional asymmetry.

**Caveat:** Depends on `normalize syntax query`. Currently mis-handles top-level `[...]` alternation (P0 bug, commit 3b8e8857). Workaround: run each alternation branch as a separate query and merge.

---

### normalize self-violations of CLAUDE.md dispatch rules

See `docs/ad-hoc-dispatch.md` for the anti-pattern definition and cross-project context.

These violate the CLAUDE.md rules "no grammar_name== branches in language-agnostic crates" and "node classification belongs in .scm". Cross-ref `docs/audit-2026-03-12.md` for prior audit findings.

~~**HIGH — `normalize-facts/src/extract.rs` ~344-356**~~

~~Three `if grammar_name == "rust"` / `"haskell"` / `("typescript" || "javascript")` branches in a language-agnostic extraction crate, each calling a bespoke post-process (`merge_rust_impl_blocks`, `dedup_haskell_functions`, `mark_interface_implementations`). Should be a `Language` trait `post_process_symbols` hook (or a `.scm` fix for the Haskell deduplication case, which may be a grammar issue rather than a post-process need). Direct CLAUDE.md violation.~~

RESOLVED: Added `post_process_symbols` to the `Language` trait (in `normalize-languages/src/traits.rs`; `InterfaceResolver` moved to `normalize-facts-core` to avoid dep-inversion). Rust/Haskell/TypeScript/TSX/JavaScript override it; the three `grammar_name ==` branches replaced by a single `support.post_process_symbols(...)` call.

~~**HIGH — `normalize-deps/src/lib.rs:50-53` + `collect_js_ts_deps` ~409-638**~~

~~JS/TS/TSX bypass the `.scm` imports path (used by all other languages via `get_imports`) for a 200+ line hand-rolled AST walker. The `.scm` files (`javascript.imports.scm`, `typescript.imports.scm`, `tsx.imports.scm`) already exist; only `require()` (CommonJS) is missing from `javascript.imports.scm`. Fix: add `require()` to the `.scm`, delete the JS/TS special-case. Violates "node classification belongs in .scm."~~

RESOLVED 2026-05-30: Added CommonJS `require()` patterns (simple binding, shorthand and aliased destructuring, bare side-effect) to `javascript.imports.scm`, `typescript.imports.scm`, `tsx.imports.scm`. Added reexport patterns (`export * from`, `export * as ns from`, `export { name } from`) to `tsx.imports.scm` (JS and TS already had them). Extended `collect_imports_from_query` to honor `@import.reexport` captures and return `(Vec<Import>, Vec<ReExport>)`. Removed the `grammar_name == "javascript"/"typescript"/"tsx"` match-arm dispatch from `DepsExtractor::extract`, and deleted `extract_javascript`, `extract_typescript`, `extract_tsx`, `extract_js_ts_deps`, `collect_js_ts_deps`, and all helper fns (`extract_require_call`, `extract_string_from_args`, `collect_destructure_names`, `collect_export_clause_names`, `collect_import_names`, `collect_variable_names`). All 9 tests pass; full clippy clean.

**~~HIGH~~ RESOLVED (for supported languages) — `normalize-refactor/src/{extract_function,add_parameter,inline_variable,introduce_variable}.rs`**

~~Pervasive `match grammar` / `grammar ==` dispatch on hardcoded language names + hardcoded node-kind strings throughout the refactor recipes.~~ Node-kind lookups are now `.scm` queries loaded via `GrammarLoader`; code *generation* moved to the `RefactorCodeGen` trait. Phase B (per-language broadening, below) is the only remaining work.

**Step 1 (RECIPE MIGRATION) DONE 2026-05-30** — `refactor(refactor): classify via refactor.scm + generate via RefactorCodeGen`. All four recipes now consume the Stage-0 foundation; ZERO `match grammar` / `grammar ==` / `grammar_name ==` and ZERO hardcoded node-kind classification lists remain in the 4 files (grep-verified). Details:
- New `refactor_query.rs` helper `RefactorCaptures::load(grammar, root, content)` runs the `refactor` query once and collects per-capture node-ID sets (mirrors how `actions.rs` consumes `decorations`); `caps.is("var_decl", &node)` replaces every node-kind-string check.
- `add_parameter`: `function_item_kinds`/`param_list_kind`/`call_kind`/`arg_list_kind`/`format_param` deleted → `@refactor.function_def`/`@refactor.param_list`/`@refactor.call`/`@refactor.arg_list` + `cg.format_param`.
- `inline_variable`: `find_declaration_node`/`is_reassignment`/`is_scope_kind`/`is_block_kind` grammar arms → `@refactor.var_decl`/`@refactor.reassign`/`@refactor.scope`. Category-C binding/initializer navigation re-keyed off the matched capture (not grammar): unified `is_binding_ident_in_var_decl` + `extract_initializer` try field-name strategies (`left`, declarator `value`/`name`, first-named-after-`=`) — extraction, allowed in Rust.
- `introduce_variable`: `is_statement_kind`/`is_block_kind`/`make_binding` → `@refactor.statement`(+`var_decl`/`reassign`/`block` for the not-an-expression check) / `@refactor.block` / `cg.render_binding`.
- `extract_function`: `generate_function`+`generate_{rust,python,go,ts,java}_function`+`generate_call_site` deleted (relocated exact-output tests live in normalize-languages, verified passing) → `cg.render_function(spec)`/`cg.render_call_site(spec)` via new `to_gen_params`/`to_gen_return` mappers. `grammar_name == "rust"` Result gate → `cg.uses_result_for_exceptions()`; multi-return `!=` chain → `!cg.supports_multi_return()`. Category-C `is_mut_binding`/`infer_type_from_annotation` moved behind two new trait methods `param_is_mutable`/`infer_param_type` (Rust + TS impls). **Fixed a latent bug**: extract_function keyed codegen on `support.name()` ("Rust"/"Python"/…) which never matched the lowercase `match grammar` arms — every non-Rust language silently fell to the Rust default. Trait dispatch keys on the real language, so it's now correct; added `supports_multi_return = true` to TS/Tsx/JS to match the documented multi-return intent.
- Unsupported languages (no `RefactorCodeGen` and/or no `*.refactor.scm`) now return a clear "X does not support language <lang>" error instead of falling through to a Rust-shaped default. New tests assert this (Go: codegen-yes/query-no) in all three of add_parameter/inline_variable/introduce_variable.
- Parity: all pre-existing recipe tests (rust/python/ts/js inline+introduce+add-param, reassignment-error, side-effects, indentation) still pass unchanged.

**ORIGINAL DESCRIPTION (for reference):** Node-kind lookups (`function_item_kinds`, `param_list_kind`, etc.) should be `.scm` queries loaded via `GrammarLoader`; code *generation* (`generate_rust_function` etc.) cannot be `.scm` — needs a `CodeGen` trait or Language-trait methods. Violated the CLAUDE.md node-classification rule.

**Step 0 (FOUNDATION) DONE 2026-05-30** — `feat(languages): refactor codegen trait + refactor.scm query purpose`. Shared infra landed; recipes NOT yet migrated (violation remains open). Delivered:
- `.refactor.scm` query purpose loaded via `GrammarLoader::get_refactor` / `bundled_refactor_query`, mirroring `decorations`. Capture vocabulary: `@refactor.function_def`, `@refactor.param_list`, `@refactor.call`, `@refactor.arg_list`, `@refactor.var_decl`, `@refactor.reassign`, `@refactor.scope`, `@refactor.statement`, `@refactor.block`. Files for rust, python, javascript, typescript, tsx — each compiled against the real installed grammar in `test_bundled_refactor_queries_compile`.
- `RefactorCodeGen: Language` capability trait + `as_refactor_codegen()` default-`None` (mirrors `LanguageSymbols`/`as_symbols`). Methods: `format_param`, `render_binding`, `render_function`, `render_call_site`, `supports_multi_return`, `uses_result_for_exceptions`. Owned spec structs `ExtractedFnSpec`/`CallSiteSpec`/`GenParam`/`GenReturn` live in `normalize-languages` (dep direction verified: refactor → languages, never reverse). Impls for Rust, Python, Go, TypeScript, Tsx, JavaScript, Java (JS/TS/TSX share `ecmascript.rs` helpers). Byte-identical to the current `generate_*` output — parity locked by relocated exact-output tests in each language's `refactor_codegen_tests` module.
- Stale node-kind found: the TS/JS recipe's `function_item_kinds` listed `"function"`, but the arborium JS/TS grammars use `function_expression` — the `.scm` uses the correct kind.

**Phase B — broadening worklist** (the recipe-migration stage migrates recipes to the new infra; THIS list is the per-language fan-out to give every GP language full refactor support). Each needs `<lang>.refactor.scm` + `RefactorCodeGen` impl. Codegen for a new language is real per-language design work (not mechanical), so each is its own staged effort. Recipe-supported today: rust, python, javascript, typescript, tsx (have both). Go + Java have `RefactorCodeGen` but still need `.refactor.scm` to become recipe targets. GP languages (those with a `module_resolver`) still needing BOTH:
- [x] go (`.refactor.scm` + codegen done)
- [x] java (`.refactor.scm` + codegen done)
- [x] ruby
- [x] kotlin
- [x] scala
- [x] groovy
- [x] c-sharp
- [x] vb (extract/introduce; `@refactor.reassign` omitted — VB grammar conflates `x = expr` with `call_statement`)
- [x] swift (extract/inline/introduce; add-parameter N/A — grammar has no single param-list node)
- [x] dart (extract/inline/introduce; add-parameter rewrites the signature only — grammar has no call-expression node to find call sites)
- [x] zig (full; `uses_result_for_exceptions` for `!T` error unions)
- [x] d (extract/inline/introduce; reassignment is best-effort — D grammar misparses bare `x = expr` statements)
- [x] lua
- [x] php

**Phase B — DEFERRED** (grammar/semantics make extract-function / add-parameter / inline-variable / introduce-variable map awkwardly; each would ship a misaligned implementation rather than honest support). Per-language rationale:
- [ ] fsharp — ML curried application + pattern-equation/value bindings. No comma-delimited param list (`argument_patterns` is space-separated) and no call-expression node (`application_expression` is curried juxtaposition), immutable so no reassignment node, expression-oriented bodies with no statement list. The recipes' paren/comma param-list + statement-sequence model does not map.
- [ ] elixir — macro-homoiconic grammar: `def`, `defmodule`, `if`, `case`, and ordinary calls are ALL `(call target: (identifier) (arguments) (do_block)?)` nodes distinguished only by the target name; `def name(params)` nests the params inside a SECOND `call` (`name(params)`), assignment is a `binary_operator`, and there are no statement nodes. Function-def/param-list/statement/reassignment distinctions the recipes need don't exist as node kinds.
- [ ] erlang — clause/pattern-equation function defs with `;`-separated clauses and no mutable bindings (single assignment); extract-function's live-out/reassignment model doesn't apply, and there's no statement-list block.
- [ ] haskell — pure pattern-equation definitions, lazy expression bodies, no statements, no reassignment. add-parameter/inline-variable map awkwardly (a binding is an equation, not a statement).
- [ ] ocaml — ML `let ... in` expression bindings, no statement list, no reassignment (refs use `:=`). Same expression-oriented mismatch as F#.
- [ ] perl — sigil-heavy, highly context-sensitive grammar; `my $x` declarations and calls are not cleanly separable into the capture vocabulary, and the grammar's expression/statement modeling is ambiguous (similar to the VB/D assignment ambiguity but pervasive).
- [ ] clojure — lisp s-expressions: every form is a `list`/`vector`; "function definition", "call", "binding" are all the same node shape differing only by the head symbol. Codegen is structural (sexpr surgery), not the line-oriented param-list/statement model the recipes use.
- [ ] commonlisp — same lisp s-expr rationale as clojure.
- [ ] scheme — same lisp s-expr rationale as clojure (R7RS).
- [ ] gleam — ML-style pattern bindings + pipe-oriented expressions; immutable, expression-bodied, no statement list. Maps awkwardly like F#/OCaml.
- [ ] rescript — ML/OCaml-derived (`let` expression bindings, curried application); same expression-oriented mismatch as OCaml/F#.
- [ ] elm — pure-functional pattern-equation defs, immutable, expression-only (no statements, no reassignment). The recipe model does not apply.
- [ ] nix — pure-functional expression language: `let ... in`, attribute sets, no statements, no reassignment, functions are single-argument lambdas (currying). Extract-function has no statement-sequence to lift.
- [ ] r — vector/stats language; function bodies are expression sequences, `<-`/`=` assignment is an operator not a distinct statement node, and extract-function is rarely meaningful for typical R analysis scripts.
- [ ] julia — scientific/array language; while it has functions and assignment, extract-function/add-parameter are rarely meaningful for the array/REPL-oriented code base it targets, and multiple-dispatch method defs complicate "the" definition. Deferred pending demand.
- [ ] matlab — array/stats language; one-function-per-file convention, command syntax, and `end`-delimited blocks without a clean statement-list node. Extract-function rarely meaningful; deferred.
- [ ] prolog — logic language: clauses/rules, not functions; there is no function-extraction or add-parameter analog (a predicate clause is a head + body of goals, not a parameterised callable in the imperative sense).

~~**MEDIUM — `normalize-filter/src/lib.rs` ~94-103**~~

~~The `build` filter alias hardcodes `target/`, `node_modules/`, `.next/`, `.nuxt/`, `__pycache__/` (Cargo/npm/Next/Nuxt/Python conventions) in a library crate. Mitigated by being applied only on explicit `build` alias request (not silently). Judgment call per CLAUDE.md: curated-default-alias vs. hardcoded-third-party-convention. CLAUDE.md says these belong in project config; however this is explicitly a *named alias* the user opts into. Flag for decision, not emergency fix.~~

WONTFIX: Filter aliases are fully overridable at runtime. `AliasConfig::get_with_languages` checks the user's `[aliases]` config first and only falls back to the builtin `match name { "build" => ... }` if the user hasn't defined that alias. Users override, extend, or disable any alias via `.normalize/config.toml` `[aliases]` (e.g. `build = [...]` to replace, `tests = []` to disable). The repo's own `.normalize/config.toml` demonstrates this (`todo = ["TODO.md"]`). Documented in `docs/cli/aliases.md`. The hardcoded paths are overridable defaults — a curated convenience alias in the same category as `config`/`docs`/`generated` — not the silent, non-configurable tool-convention coupling the CLAUDE.md Hard Constraint targets.

**LOW — `normalize-filter/src/service.rs` ~114, ~123 (standalone binary + alias config)**

The standalone `normalize-filter` binary calls `AliasConfig::default()` and does not load project `.normalize/config.toml`, so alias overrides defined there do not reach the standalone binary (only the main `normalize` CLI loads project config and passes `config.aliases` to the filter). May be intentional — a standalone binary with no config discovery. Decide: should the standalone binary also walk up for a project config, or is its current "no config" behavior correct? Not a confirmed bug.

~~**LOW / HYPOTHESIS — `normalize-ecosystems/src/local_docs.rs` ~278**~~

~~`walk_rs_files` hardcodes the `.rs` extension (also `lib.rs`/`main.rs`/`mod.rs` candidates). The struct is Cargo-specific, so Rust-only coupling may be intentional and correct.~~

WONTFIX: `CargoLocalDocsExtractor` is intentionally Cargo/Rust-scoped (module doc + `cargo metadata`); hardcoding `.rs`/`lib.rs`/`mod.rs` inside a Cargo-specific extractor is correct, not a violation.

~~**LOW — `normalize-facts/tests/extract_fixtures.rs` ~217-270**~~

~~Hardcoded runtime→command map (`python`→`python3`, etc.) in tests. Test-only, out of the library-crate rule's scope.~~

WONTFIX: Test plumbing mapping a language to its interpreter binary is environment configuration, not library-crate classification; the dispatch-rule does not apply to test harnesses.

~~**INTERMITTENT — `normalize-facts/tests/extract_fixtures.rs` (javascript `add_numbers` import mismatch)**~~

RESOLVED (2026-07-02): not a runtime race — a stale fixture. The JS `add_numbers`
`expected/imports.json` was `[]`, predating the commits that routed JS imports through
the generic `.scm` query path (13ffe6a5) and added CommonJS `require` patterns to
`javascript.imports.scm` (db7ec044). Once those landed, extraction correctly reports
`const { Calculator } = require("./math")` as an import; runs before/after the code change
in-session flipped the result, which read as "intermittent." Extraction itself is
deterministic (Vec-preserving dedup, deterministic tree-sitter cursor). Fix: regenerated
the fixture to expect the `Calculator` import. Verified stable green over 30 runs (20
default-thread + 10 single-thread).

---

### Code Quality

- Unnecessary aliases: `let x = Foo; x.bar()` → `Foo.bar()`. Lint for pointless intermediate bindings.
- PR/diff analysis: `normalize analyze --pr` or `--diff` for changed code focus (needs broader analysis workflow design)
- Deduplicate SQL queries in normalize: many ad-hoc queries could use shared prepared statements or query builders (needs design: queries use different execution contexts - Connection vs Transaction)
- Detect reinvented wheels: hand-rolled JSON/escaping when serde exists, manual string building for structured formats, reimplemented stdlib. Heuristics unclear. Full codebase scan impractical. Maybe: (1) trigger on new code matching suspicious patterns, (2) index function signatures and flag known anti-patterns, (3) check unused crate features vs hand-rolled equivalents. Research problem.
- Remaining duplicate/clone detection improvements:

### normalize-typegen

**Input Parsers:**
- [x] ~~Protobuf parser - read .proto files to IR~~ — done; `src/input/proto.rs`, hand-rolled tokenizer (no arborium proto grammar); messages→structs, enums→int-literal enums, `repeated`→arrays, `map<K,V>`→`Type::Map`, `optional`→optional; always available, no feature flag
- [x] ~~GraphQL schema parser - read GraphQL SDL to IR~~ — done; `src/input/graphql.rs` (feature `input-graphql`), uses arborium-graphql tree-sitter; `type`/`input`/`interface`→structs, `enum`→string-literal enum, non-null `!`→required, nullable→`Type::Optional`

**Output Backends:**
- [x] ~~JSON Schema output~~ — done; `src/output/jsonschema.rs` (feature `backend-jsonschema`); emits draft 2020-12 with `$defs`, `$ref`, `anyOf`/`oneOf`, `required` arrays, `additionalProperties: false`; respects `nullable`, constraints, defaults, docs
- [x] ~~GraphQL SDL output~~ — done; `src/output/graphql.rs` (feature `backend-graphql`); structs→`type`/`input`, string enums→`enum` with UPPER_CASE, tagged unions→`union` + helper types, `nullable`+`required` → non-null `!` vs nullable
- [x] ~~Protobuf output~~ — done; `src/output/proto.rs` (feature `backend-proto`); emits proto3; structs→`message`, string enums with `_UNSPECIFIED=0` entry, int enums, tagged unions→`message` with `oneof`, arrays→`repeated`, optional fields use explicit `optional` keyword

**CLI Enhancements:**
- [x] ~~Multiple output files (`--split` to emit one file per type)~~ — done; `--split` added to `normalize generate types`; requires `--output` dir; emits one file per `TypeDef` with snake_case filename derived from type name; `type_name_to_filename` handles PascalCase→snake_case conversion
- [x] ~~Dry-run mode (`--dry-run` to preview without writing)~~ — done; `--dry-run` added to `normalize generate types`; prints `--- filename ---\n<content>` for each file that would be written without touching disk; works with both normal and `--split` modes

**IR Improvements:**
- [x] ~~Validation~~ — done; `Schema::validate()` checks: valid identifiers, no duplicate type/field names, all `Ref` targets resolve, circular reference detection via DFS; returns `Vec<ValidationError>`
- [x] ~~Nullable vs Optional distinction~~ — done; `Field::nullable: bool` added (distinct from `required`); `nullable()` builder method; backends (GraphQL, JSON Schema) use it
- [x] ~~Default values support in Field~~ — done; `Field::default: Option<DefaultValue>` with `String`/`Number`/`Bool`/`Null` variants; `with_default()` builder; JSON Schema backend emits `"default"` keyword
- [x] ~~Constraints (min/max, pattern, format)~~ — done; `Field::constraints: Option<FieldConstraints>` with `min`, `max`, `min_length`, `max_length`, `pattern`, `format`; `with_constraints()` builder; JSON Schema backend maps to `minimum`/`maximum`/`minLength`/`maxLength`/`pattern`/`format`

### normalize-surface-syntax

**Readers:**
- TypeScript reader: ~~missing classes/interfaces/type annotations, spread/destructuring, template literals, async/await~~ — done: classes lowered to function + prototype assignments, interfaces skipped, ~~type annotations ignored~~ type annotations now preserved in `Param::type_annotation`/`Function::return_type`/`Stmt::Let::type_annotation`, ~~template literals lowered to Concat~~ template literals now produce `Expr::TemplateLiteral`, destructuring lowered, rest params handled, await lowered to inner expr, new_expression lowered to call
- ~~Lua reader: missing metatables/metamethods, string methods (`:method()` syntax)~~ — done: metamethod keys recognized as identifier keys; `obj:method(args)` desugared to `obj.method(obj, args)` with implicit self; `["string"]` computed keys extract string value; multi-variable generic for captures all vars; numeric for step uses grammar step field; elseif chaining bug fixed
- [x] JavaScript reader — added `javascript.rs` using `arborium-javascript` + shared `ReadContext` via `read_with_language`; feature-gated as `read-javascript`

**Writers:**
- ~~Lua writer: verify idiomatic output (use `and`/`or` vs `&&`/`||`), string escaping edge cases~~ — done: already correct for `and`/`or`/`not`/`~=`/`nil`; object keys now use bare identifier syntax for valid Lua identifiers; string escaping now handles null bytes; for-in no longer prepends hardcoded `_, `
- ~~TypeScript writer: type annotations, semicolon placement verification, template literal output~~ — done: semicolons verified; type annotations now emitted (`: type` on params/vars, `: return_type` on functions); template literals now emitted as backtick syntax; comments emitted.
- [x] JavaScript writer — added `javascript.rs` delegating to `TypeScriptWriter`; feature-gated as `write-javascript`

**Testing:**
- ~~Edge case tests: nested expressions, complex control flow, Unicode strings~~ — done: added reader tests for nested calls, multi-return, Unicode, long strings, numeric for with/without step, generic for multi-var, method call self-desugaring, metamethod keys, computed string keys, complex elseif; added writer tests for Lua idioms, object key emission, string escaping, unicode, for-in multi-var

**IR Improvements:**
- [x] ~~Comments preservation (for documentation translation)~~ — done: `Stmt::Comment { text, block, span }` added to IR; builders `Stmt::comment_line(text)` and `Stmt::comment_block(text)`; TypeScript reader parses `// line`, `/* block */`, `/** JSDoc */` comments; Lua reader parses `-- line`, `--- LuaDoc`, `--[[ block ]]` comments; TypeScript writer emits `//`/`/* */`/`/** */` (JSDoc multi-line when `block && text.contains('\n')`); Lua writer emits `--`/`--[[ ]]`; Python writer emits `#`/`"""..."""`; s-expr serializes as `["std.comment_line", text]`/`["std.comment_block", text]`; `StructureEq` compares `text` + `block`; `with_span()` supported
- [x] ~~Source locations (for error messages, debugging)~~ — done; `Span { start_line, start_col, end_line, end_col }` added to `ir/mod.rs` (1-based lines, 0-based cols); `span: Option<Span>` added to structured `Stmt` variants (`Let`, `If`, `While`, `For`, `ForIn`, `TryCatch`) and `Expr` variants (`Binary`, `Unary`, `Call`, `Member`, `Conditional`, `Assign`); `Span::from_ts()` converts tree-sitter `Point`; `with_span()` builder on both types; writers ignore spans; `StructureEq` ignores spans
- [x] ~~Import/export statements~~ — done: `Stmt::Import { source, names: Vec<ImportName> }` and `Stmt::Export { names: Vec<ExportName>, source }` added to IR; `ImportName { name, alias, is_namespace }` and `ExportName { name, alias }` types with builders (`named`, `aliased`, `namespace`, `default`); TypeScript reader parses `import_statement` (named, namespace, default, side-effect) and `export_statement` (named export, re-export, exported class/function) into first-class IR nodes; Python reader parses `import_statement` and `import_from_statement`; TypeScript writer emits `import { X } from 'z'` / `import * as ns from 'z'` / `export { X as Y }`; Python writer emits `import X` and `from X import Y`; Lua writer lowers to `require()` calls; s-expr serializes as `["std.import", source, names]` / `["std.export", names]`; `StructureEq` handles both new variants
- [x] ~~Class definitions, method definitions (IR-level; currently lowered to functions + prototype assignments)~~ — done: `Stmt::Class { name, extends, methods: Vec<Method> }` and `Method { name, params, body, is_static, return_type }` added to IR; TypeScript reader: `class_declaration` → `Stmt::Class` with all `method_definition` nodes (static detection, return types); Python reader: `class_definition` → `Stmt::Class` including `@staticmethod`; TypeScript writer emits `class Foo extends Bar { method(): T { ... } }`; Python writer emits `class Foo(Bar):\n    def method(self): ...`; Lua writer lowers to metatable pattern (`local Foo = {}; Foo.__index = Foo; function Foo.new() ... function Foo:method() ...`); class expressions still lowered to function expressions; s-expr serializes as `["std.class", name, base, methods]`; `StructureEq for Method` added
- [x] ~~Type annotations~~ — done: `Param { name, type_annotation: Option<String> }` added; `Function::return_type: Option<String>` added; `Stmt::Let::type_annotation: Option<String>` added; TypeScript reader populates all three from `type_annotation` nodes and `return_type` field; Python reader populates `Param::type_annotation` from `typed_parameter` nodes and `Function::return_type` from `return_type` field; TypeScript/Python writers emit annotations in language-appropriate syntax; Lua writer ignores annotations; `StructureEq` treats type annotations as surface hints (ignored in comparison). Template literals: `Expr::TemplateLiteral(Vec<TemplatePart>)` added; `TemplatePart::Text(String)` and `TemplatePart::Expr(Box<Expr>)`; TypeScript reader produces `TemplateLiteral` instead of `Concat` chains; TypeScript writer emits backtick syntax; Python writer emits f-strings; Lua writer lowers to `..` concatenation; s-expr serializes as chained `str.concat` calls.
- [x] ~~Pattern matching / destructuring (IR-level; currently lowered at read time)~~ — done: `Pat` enum (`Ident(String)`, `Object(Vec<PatField>)`, `Array(Vec<Option<Pat>>, Option<String>)`, `Rest(Box<Pat>)`) and `PatField { key, pat, default }` added to `ir/pat.rs`; `Stmt::Destructure { pat, value, mutable }` added to `Stmt`; TypeScript reader now produces `Stmt::Destructure` instead of lowering `object_pattern`/`array_pattern` to individual `Stmt::Let` bindings — full `read_pat` method handles shorthand fields, renamed fields (`{ b: c }`), nested patterns, assignment defaults, and rest elements; TypeScript writer emits `const { a, b: c } = obj` and `const [x, y, ...rest] = arr` with proper `{ }` spacing; Python reader handles `pattern_list`, `tuple_pattern`, `list_pattern` and `list_splat_pattern` (star unpacking) as `Stmt::Destructure`; Python writer emits `a, b = expr` (tuple syntax); Lua writer lowers to `local a, b = table.unpack(expr)`; s-expr lowers to `std.let` bindings; `StructureEq` added for `Pat`, `PatField`, and `Stmt::Destructure`; `Pat`/`PatField` re-exported from crate root; round-trip tests for TypeScript and Python

### Package Index Backlog (simplest → complex)

**6. Crates.io db-dump** (hard - large dataset)
- [ ] Download and parse db-dump.tar.gz (~800MB compressed)
- [ ] ~150k crates, refreshed daily
- [ ] Would need tar + csv parsing

**7. npm registry** (hard - massive scale)
- [ ] Evaluate registry replicate API feasibility
- [ ] ~3M packages, would need incremental CouchDB replication
- [ ] Consider: is this worth implementing? Search/fetch covers most use cases

**8. PyPI bulk** (hard - no native API)
- [ ] Research alternatives to simple index scraping
- [ ] Consider BigQuery dataset (google-cloud-pypi) or warehouse.pypi.org
- [ ] Consider: is this worth implementing? fetch() covers most use cases

### Distribution

- Wrapper packages for ecosystems: npm, PyPI, Homebrew, etc.
  - Auto-generate and publish in sync with GitHub releases
  - Single binary + thin wrapper scripts per ecosystem
- Direct download: platform-detected link to latest GitHub release binary (avoid cargo install overhead)

### Fix System: Structural Rewrites (post text-replacement)

- **Sexpr-based fix expressions**: The current `fix = "template $capture"` is text replacement. For structural transforms (indentation-aware, composable), consider expressing fixes as output tree patterns rather than strings. eglint (~/git/eglint) does this for TypeScript — useful prior art for the approach even though it's TS-compiler-specific and doesn't port directly.
- **Fix fixture tests**: Infrastructure added (`fix.<ext>` + `fix.expected.<ext>` in fixture dirs; temp dir created inside fixture dir for Cargo.toml walk-up). `rust/chained-if-let` covered. Adversarial cases (nested violations, near-EOF, overlapping) not yet added. Deletion rules (`breakpoint`, `binding-pry`, `console-log`) had `fix = ""` removed — auto-delete is too aggressive for statements that may be intentional.
- **eglint findings**: ~/git/eglint — reference-based AST formatting (not tree-sitter). Core insight: IndentNode/NewlineNode carry `deltaIndent` so indentation is computed at stringify time, not baked into captured text. InterchangeableNode/ForkNode for multiple formatting options avoids explicit conflict resolution. Would require language-specific pretty-printers to adopt — non-trivial.

---

## Aspirational — Research / Long-term Vision

### normalize as LSP server

- [ ] Implement core LSP methods backed by normalize's own reference resolution:
      `textDocument/references`, `textDocument/rename`, `textDocument/definition`,
      `textDocument/documentSymbol`, `workspace/symbol`
- [ ] LSP proxy mode: `normalize serve lsp --proxy 'rust-analyzer'` — forward requests to
      an arbitrary LSP command, use normalize as fallback or supplement
- [ ] Editor integration: VS Code extension, Neovim config — use normalize LSP for languages
      without a native server, proxy for languages that have one

### Deep Analysis (CodeQL-style)

- [ ] Type extraction for top languages (TS, Python, Rust, Go)
- [ ] **Inter-procedural dataflow / taint analysis** — highest-value next direction. The CFG
  (`normalize-cfg`), Datalog/ascent engine, and call/import facts (`view trace` does value
  provenance over the call graph) already exist. What's missing is inter-procedural
  reaching-definitions and source→sink taint relations in the fact schema. Adding these
  surfaces the existing substrate into the headline capability that defines CodeQL/Semgrep
  deep mode, with no new infrastructure required.
- [ ] Taint tracking (subsumed by the dataflow item above once relations exist)
- Note: significant per-language effort for type extraction, but tractable with LLM assistance

### Trait-Based Extensibility

All trait-based crates follow the normalize-languages pattern for extensibility:
- Global registry with `register()` function for user implementations
- Built-ins initialized lazily via `init_builtin()` + `OnceLock`
- No feature gates (implementations are small, not worth the complexity)

Pattern: traits are the extensibility mechanism. Users implement traits in their own code, register at runtime. normalize CLI can add Lua bindings at application layer for scripting.

### Agent Future

Core agency features complete (shadow editing, validation, risk gates, retry, auto-commit).

**Remaining:**
- [ ] Test selection: run only tests affected by changes (use call graph). Related: `analyze test-gaps` (see `docs/design/test-gaps.md`) shares the test-context classification
- [ ] Task decomposition: break large tasks into validated subtasks
- [x] Cross-file refactoring: rename symbol across codebase
- [ ] Partial success: apply working edits, report failures
- [ ] Human-in-the-loop escalation: ask user when stuck

**RLM-inspired** (see `docs/research/recursive-language-models.md`):
- [ ] Recursive investigation: agent self-invokes on subsets (e.g., `view --types-only` → pick symbols → `view symbol` → recurse if large)
- [ ] Decomposition prompting: system prompt guides "search before answering" strategy
- [x] Chunked viewing: `normalize view chunk <file> --chunk N` or `--around "pattern"` for large files
- [ ] REPL-style persistence: extend ephemeral context beyond 1 turn for iterative refinement
- [ ] Depth/cost limits: cap recursion depth, token budgets per investigation

### Agent / MCP

- Gemini Flash 3 prompt sensitivity: certain phrases ("shell", "execute", nested `[--opts]`) trigger 500 errors. Investigate if prompt can be further simplified to avoid safety filters entirely. See `docs/design/agent.md` for current workarounds.
- `normalize @agent` MCP support as second-class citizen
  - Our own tools take priority, MCP as fallback/extension mechanism
  - Need to design how MCP servers are discovered/configured
- Context view management: extend/edit/remove code views already in agent context
  - Agents should be able to request "add more context around this symbol" or "remove this view"
  - Incremental context refinement vs full re-fetch

### Session Analysis Backlog

**Replace `--sort-by-*` flags with `--sort <field>`:**
- [x] `--sort-by-x` proliferates flags. Replace with a single `--sort <field>` accepting composite sorts: `--sort -tokens,+session` (`-`=desc, `+`=asc, prefix omitted = sensible default). Sensible defaults: numeric→desc, string→asc, date→desc (except message/event sequences where chronological order is natural, so date→asc). Applies everywhere sort is exposed (sessions, tools, etc.). Note: no `--sort-by-*` flags existed; `--sort <field>` was already the interface. This item is done.

- [x] **Tool sequence filtering (`--sequence`):** `normalize sessions messages --sequence Grep,Grep,Read` — returns turns where consecutive tool calls match the pattern (case-insensitive prefix match), with `--context-turns N` surrounding context. Answers the "frequency vs motivation" gap: transition matrix shows how often, sequence filter shows what actually happened.

**Composable message filters:**
- [x] `--has-tool <name>` — messages in turns that used a specific tool (case-insensitive prefix match on tool name)
- [x] `--min-chars <N>` / `--max-chars <N>` — filter by message length
- [x] `--errors-only` — turns with tool errors (ToolResult is_error=true)
- [x] `--turn-range <start>-<end>` — positional filtering within sessions (e.g. `--turn-range 5-10`)
- [x] `--exclude-interrupted` — skip messages containing `[Request interrupted by user]`

**Analysis features:**
1. [x] **Cross-repo comparison**: `normalize sessions stats --by-repo` — groups sessions by repository, shows per-repo breakdown: session count, turns, tokens_in/out, error rate, parallelization rate, cost. Sorted by total tokens desc.
2. [x] **Ngram analysis**: `normalize sessions ngrams [session-id] [--n N] [--top K] [--role assistant|user|all]` — extracts word n-grams (bigrams by default) from message text, shows top-K most frequent. Useful for finding repeated error messages, boilerplate responses.
3. [x] **Parallelization hints**: `normalize sessions parallelization [session-id]` — shows turns with sequential same-type tool calls that could be parallelized. `--threshold N` (default 2) minimum group size. Example: `Turn 12: Could parallelize: Read(foo.rs) → Read(bar.rs) → Read(baz.rs)`
4. [x] **File edit heatmap**: `normalize sessions heatmap [session-id]` — per-file read/write counts, classifies as `hot` (>5 writes), `read_only` (0 writes = potential test gap), `normal`. `--top N` (default 20), sorted by write_count desc.
5. [x] **Cost breakdown**: `normalize sessions cost [session-id]` — per-turn token counts and estimated USD cost using model-specific pricing; summary shows total cost, cache savings, cache efficiency %.

**Other session analysis:**
- Web syntax highlighting: share tree-sitter grammars between native and web SPAs
  - Option A: embed tree-sitter WASM runtime, load .so grammars
  - Option B: `/api/highlight` endpoint, server-side highlighting
- Antigravity conversations: `~/.gemini/antigravity/conversations/*.pb` (protobuf - needs schema, files appear encrypted)
- Antigravity brain artifacts: `~/.gemini/antigravity/brain/*/` (task/plan/walkthrough metadata)
- Additional agent formats (need to find log locations/formats):
  - Windsurf (Codeium)
  - Cursor
  - Cline
  - Roo Code
  - Gemini Code Assist (VS Code extension)
  - GitHub Copilot (VS Code)
- Better `--compact` format: key:value pairs, no tables, all info preserved
- Better `--pretty` format: bar charts for tools, progress bar for success rate
- [x] `normalize sessions mark <id>` / `unmark <id>`: mark/unmark as reviewed (stored in `.normalize/sessions-reviewed`); `--reviewed`/`--unreviewed` filter flags on `sessions list`
- [x] **Project sync / portability** (`normalize sync <dest>`): implemented. Uses `walkdir` +
  `std::fs::copy` (note: `fast_rsync` is a delta algorithm library, not a file copier).
  Copies project dir + session metadata via `project_metadata_roots`, rewrites index paths
  post-copy via libsql. `--dry-run`, `--verbose`, `--all`, `--active N`, `--repo`/`--exclude` done.
  - [ ] Follow-up: incremental sync (skip unchanged files via mtime/checksum)
  - [ ] Follow-up: multi-format session discovery (currently Claude Code only)
- Agent habit analysis: study session logs to identify builtin vs learned behaviors
  - Example: "git status before commit" - is this hardcoded or from CLAUDE.md guidance?
  - Test methodology: fresh/empty repo without project instructions
  - Cross-agent comparison: Claude Code, Gemini CLI, OpenAI Codex, etc.
  - Goal: understand what behaviors to encode in normalize agent (model-agnostic reliability)
  - Maybe: automated agent testing harness (run same tasks across assistants)

### Cross-file Context Construction (open research question)

How should a SWE agent handle edits that require understanding module A to correctly edit module B? The index has the dependency graph, but we don't have a principled answer for context budget allocation across subtasks. Related to Pillar 2 (semantic refactoring) and Pillar 4 (incremental): a good answer probably involves the daemon pre-loading transitive context for a given edit target, so the agent doesn't have to re-read it. No concrete plan yet — needs more thought.

### Friction Signals (see `docs/research/agent-adaptation.md`)

How do we know when tools aren't working? Implicit signals from agent behavior:
- Correction patterns: "You're right", "Should have" after tool calls
- Long tool chains: 5+ calls without acting
- Tool avoidance: grep instead of normalize, spawning Explore agents
- Follow-up patterns: `--types-only` → immediately view symbol
- Repeated queries: same file viewed multiple times

### Large File / Complexity Diagnostics — Open Question

`normalize rules run` runs in the pre-commit hook and can flag large files and high
complexity. Open questions:
- Are large-file violations currently errors (blocking) or warnings in the pre-commit?
- Should they be errors? Pre-commit is a safety net but late — the agent has already
  written, re-read, and worked with the file by then.
- Earlier signal: `PostToolUse` hook on Edit/Write that injects file-size into
  `additionalContext` immediately after the edit. Actionable at creation time.
- CC has no LSP, so the LSP diagnostic path doesn't help for agent workflows.
- The right answer may be: errors in pre-commit (blocking) + hook for early warning.
  Pre-commit already has `normalize rules run` — check whether large-file rules are
  wired into it and at what severity.

### Workspace/Context Management

- Persistent workspace concept (like Notion): files, tool results, context stored permanently
- Cross-session continuity without re-reading everything
- Investigate memory-mapped context, incremental updates

### Package Management

- `normalize package install/uninstall`: proxy to ecosystem tools (cargo add, npm install, etc.)
  - Very low priority - needs concrete use case showing value beyond direct tool usage
  - Possible value-adds: install across all ecosystems, auto-audit after install, config-driven installs

### Workflow Engine

- JSON Schema for complex action parameters (currently string-only)
- Workflow chaining: automatically trigger next workflow based on outcome (e.g., Investigation → Fix → Review)

### Script System

- TOML workflow format: structured definition (steps, actions) - **deferred until use cases are clearer**
  - Builtin `workflow` runner script interprets TOML files
  - Users can also write pure Lua scripts directly
- Lua test framework: test discovery for `.normalize/tests/` (test + test.property modules done)
  - Command naming: must clearly indicate "normalize Lua scripts" not general testing (avoid `@test`, `@spec`, `@check`)
  - Alternative: no special command, just run test files directly via `normalize <file>`
- Type system uses beyond validation
  - Done: `T.describe(schema)` for introspection, `type.generate` for property testing
  - Future: extract descriptions from comments (LuaDoc-style) instead of `description` field
- Format libraries (Lua): json, yaml, toml, kdl - **very low priority, defer until concrete use case**
  - Pure Lua implementations preferred (simple, no deps)
  - Key ordering: sort alphabetically by default, `__keyorder` metatable field for explicit order

### Vision

- **Friction Minimization Loop**: normalize should make it easier to reduce friction, which accelerates development, which makes it easier to improve normalize. Workflows documented → failure modes identified → encoded as tooling → friction reduced → faster iteration. The goal is tooling that catches problems automatically (high reliability) not documentation that hopes someone reads it (low reliability).
- Verification Loops: domain-specific validation (compiler, linter, tests) before accepting output
- Synthesis: decompose complex tasks into solvable subproblems (`normalize synthesize`)
- Plugin Architecture: extensible view providers, synthesis strategies, code generators

### First Release

```bash
git tag v0.1.0
git push --tags
```
- Verify cross-platform builds in GitHub Actions
- Test `normalize update` against real release
- VS Code extension: test and publish to marketplace (after first CLI release)

### 0.2.0 — "Coherent surface, reliable index"

**Theme:** The CLI is small enough to hold in working memory. The index is reliable enough
to depend on. The LSP is useful day-to-day.

**Already done (since 0.1.0):**
- Qualified import resolution + `callee_resolved_file` in facts (schema v6)
- Two-tier LSP diagnostics (immediate syntax, debounced fact rules)
- Incremental index updates on save (`FileIndex::update_file()`)
- Compiled query caching in `GrammarLoader`
- Language coverage audit: 47/84 languages at 100% .scm coverage; all feasible gaps filled
- `RankEntry` trait + `format_ranked_table()` — shared rendering for 13 rank commands
- `--diff <ref>` on all 12 rank commands
- Progress bars for `structure rebuild`, `analyze duplicates`, `analyze architecture`
- Per-subcommand excludes in config
- `rules recommended` field + `normalize init --setup` interactive wizard (tag grouping, impact labels, batch ops — done 2026-03-15)
- 30+ new syntax rules (Java, C/C++, C#, Kotlin, Swift, PHP)
- [x] `normalize daemon watch` — file change streaming (2026-03-15)
- [x] File-level reverse-dep tracking in daemon (2026-03-15)
- [x] Duplicate detection: cross-file same-body-pattern suppression (2026-03-15)
- [x] `normalize-rules-loader` folded into `normalize-rules` (2026-03-15)
- [x] `normalize rank` introduced; 20 ranked commands migrated from `analyze` (2026-03-16)
- [x] `normalize analyze all` deleted; `analyze node-types` moved to `syntax` (2026-03-16)
- [x] 4 trend commands merged into `normalize analyze trend-metric --metric <...>` (2026-03-16)
- [x] CLI command organization ADR written (2026-03-16)

**Remaining before 0.2.0:**

*LSP / index (from P0):*
- [x] Wire incremental Datalog evaluation: `CachedRuleEngine`, `prime_rule_engine`,
  `run_rule_incremental`, and `run_rule_with_cache` added to `normalize-facts-rules-interpret`.
  `collect_fact_diagnostics_incremental` in `normalize-rules/src/runner.rs` uses a process-level
  `ENGINE_CACHE` (keyed by root + rule_id) to retract only changed-file facts and re-derive.
  Daemon integration point documented with a TODO comment in `daemon.rs::refresh_root` pointing
  to `collect_fact_diagnostics_incremental` with `Some(&watched.last_affected)`.
  Daemon wired: after each index refresh, daemon calls `collect_fact_diagnostics_incremental`
  with `Some(&watched.last_affected)` to warm the `ENGINE_CACHE` — next `normalize ci` or
  `normalize rules run` in the same process uses the incremental path automatically.
  **Remaining:** fix JIT string comparison bug in ascent-interpreter to make eval fast.
- [x] Fix JIT string comparison bug in ascent-interpreter and re-enable `SharedJitCompiler`
  in `run_rules_source` / `run_rules_batch`. Fixed in ascent-interpreter 0.2.0-alpha.1;
  JIT re-enabled in normalize-facts-rules-interpret default features (2026-04-26).

*CLI surface (from P1):*
- [x] `view` refactor phase 1: graph navigation + history as subcommands — done 2026-03-16
- [x] `view` refactor phase 2: dissolve `ViewOutput` enum into `ViewReport` + `view list` — done 2026-03-16
- [x] `normalize view <file>` surfaces module-level doc comments as preamble — Rust `//!`, Python docstrings, Go package comments, JS/TS JSDoc, Ruby leading `#` — done 2026-03-26

*CI readiness (from P1 — see "CI readiness" section above):*
- [x] `normalize ci` command — `--no-syntax/native/fact`, `--strict`, `--sarif`, `-p <path>`, graceful index-not-built handling
- [x] Install script (curl | sh) — SHA256 verification, `NORMALIZE_VERSION` pinning, `~/.local/bin` default
- [x] CI documentation (`docs/ci.md`) — GitHub Actions/GitLab snippets, ratchet bootstrap workflow, SARIF output
- [x] Polish pass — version string, --help accuracy, config parse warning, view error messages,
  stale --engine flag references in docs. **Exit code 1 vs 2 differentiation (violations vs
  setup errors) requires server-less `ExitCode`-carrying error type — deferred post-0.2.0.**

*Release mechanics:*
- [x] Bump all crate versions to 0.2.0 in Cargo.toml files
- [x] Tag and push `v0.2.0`
- [ ] Verify `normalize update` works against a real GitHub release (cross-platform smoke test)

**Not blocking 0.2.0:**
- Comprehensive language fixtures (explicitly long-term)
- Semantic rules system (separate infrastructure, post-0.2.0)
- Shadow worktree / namespace-qualified lookups (low priority)

---

### 0.3.0 — "Understand and refactor, fast"

**Theme:** normalize becomes the tool you reach for when you need to understand a codebase
and make a cross-cutting change safely. The index is no longer just for analysis — it backs
precise multi-file edits. Linting grows a semantic tier (18 fact rules already exist; the
gap is polish + new rules, not infrastructure). Everything is incremental: no cold rebuilds,
no full re-evaluations on every invocation.

**Pillar 4 — Incremental everything**

The daemon is running but CLI invocations don't route through it — every `normalize rules run`
is a cold eval, every `structure rebuild` re-indexes the world. The goal: make the fast path
the default path.

- [x] **Incremental index** — on `structure rebuild`, only re-index files changed since the
  last build (mtime/hash based). Full rebuild only when schema changes or forced with `--full`.
- [x] **CLI → daemon routing** — `normalize rules run` (and `normalize ci`) should talk to
  the running daemon and get the pre-warmed Datalog cache instead of cold-evaluating. If no
  daemon is running, fall back to cold eval transparently.
- [x] **Incremental syntax rules** — mtime-based per-file cache in `.normalize/syntax-cache.json`; nanosecond precision; invalidates on rule set change. Done in `crates/normalize-syntax-rules/src/runner.rs`.
- [x] **stale/missing-summary cold-cache batch pass** — replaced per-directory O(dirs × history) git
  walks with a single O(history) pass in `git_batch_commit_stats`. Cold-cache run: 128s → 3.4s.
  Warm-cache (cached HEAD) run: already fast at ~2.2s. Root cause of 5+ min runs was `.claude/`
  worktrees (5190 dirs) not excluded from walker — fixed by `.gitignore` + `[walk] exclude`.
- [x] **Incremental native rules** — stale-summary already does this: when HEAD moves, `git_incremental_commit_stats` walks only new commits and updates only dirs touched in those commits (`stale_summary.rs` L676-701).
- [x] **Persistent query cache** — store per-file tree-sitter query results in the SQLite index
  so repeated `normalize view`, `normalize rank`, etc. don't re-parse unchanged files.
  Implemented in `Extractor::extract_with_support` via a `symbol_cache()` singleton that reuses
  the existing CA cache DB (`~/.config/normalize/ca-cache.sqlite`). Key: `(blake3(content),
  "symbols-v1-{all|public}", grammar_name)`. Cross-file resolver results (TS/JS interface
  resolution) are not cached. `gc_stale_versions` now preserves `"symbols-*"` entries.

**Pillar 5 — Perf and memory baseline**

Before optimizing, measure. We have no perf benchmarks and no memory budget. The malloc
crash in the pre-commit hook is a warning sign.

- [x] Establish wall-clock benchmarks for the hot paths: `structure rebuild`, `rules run`,
  `view`, `rank`. Run against a mid-size real repo (normalize itself is a good target).
  Track in CI so regressions are caught. (`benches/` crate added with criterion; baseline TBD values in `docs/perf-baseline.md`)
- [ ] Profile memory usage of a full `structure rebuild` and `rules run --engine fact` on
  normalize. Identify the top allocators. The pre-commit malloc crash suggests at least one
  path has unbounded allocation.
- [ ] Set memory budgets per command and enforce them in tests (e.g. `structure rebuild`
  on normalize should not exceed N MB RSS).

**Current state (post-0.2.0 audit):**
- `normalize view` already has full graph navigation: `referenced-by`, `references`,
  `dependents`, `trace`, `graph` are wired and functional. The "fold call-graph into view"
  item from the 0.2.0 design is already done.
- `commands/find_references.rs` exists but is not exposed as `normalize refs` — just needs
  wiring.
- 18 fact rules (Datalog) already exist: `circular-deps`, `dead-api`, `unused-import`,
  `god-file`, `god-class`, `orphan-file`, `duplicate-symbol`, `fan-out`, `hub-file`,
  `layering-violation`, `long-function`, and more. Semantic rules infrastructure is mature.
- ~~`normalize-facts-rules-builtins/src/circular_deps.rs`~~ (removed 2026-03-27) — the entire `normalize-facts-rules-builtins` crate was orphaned (no workspace members entry, no dependents). The Datalog version in `builtin_dl/circular_deps.dl` is what runs.
- Incremental evaluation API (`run_rules_source_incremental`) is implemented but not wired
  into any CLI call path. JIT re-enabled (fixed in ascent-interpreter 0.2.0-alpha.1).

**Pillar 1 — `analyze` dissolution**

`normalize view` has absorbed graph navigation. `normalize rank` has absorbed 21 ranking
commands. `analyze` still hosts 19 commands that don't fit either:

- [x] Trend commands (`complexity-trend`, `length-trend`, `density-trend`, `test-ratio-trend`)
  — moved to `normalize trend` top-level subcommand: `trend complexity`, `trend length`,
  `trend density`, `trend test-ratio`, `trend multi` (all metrics). (2026-03-28)
- [ ] Synthesis commands (`architecture`, `summary`, `health`, `coupling-clusters`,
  `cross-repo-health`) — big-picture, not a ranked list. Find the unifying trait or leave
  in `analyze` until the pattern is clear. Don't force a home.
- [x] `length` → moved to `rank length` (2026-03-28)
- [x] `test-gaps` → moved to `rank test-gaps` (2026-03-28)
- [x] `node-types` → removed from `analyze`; `syntax node-types` already existed (2026-03-28)
- [ ] Residual commands (`activity`, `docs`, `security`, `skeleton-diff`,
  `repo-coupling`, `all`) — audit each: belongs in rank/view/rules
  or stays as standalone?
- [ ] Once all commands have a proper home, `analyze` dissolves. Don't rush this — clarity
  matters more than speed.

**Pillar 2 — Semantic refactoring**

Building blocks are all present. Composition layer landed — `normalize-refactor` crate provides the engine:

- [x] `normalize refs` absorbed into `view referenced-by` — `CallEntry.access:
  Option<String>` field added (values: `"read"`/`"write"`/`"read-write"`); currently
  always `None` pending index + scope engine changes below.
- [x] **Populate `access` in `CallEntry`** — `calls` table has `access TEXT` column (schema v7); `@call.write` capture in Rust `.scm` files populates it; `view referenced-by` displays `[read]`/`[write]`/`[read-write]`. Other languages: extend `.scm` files when grammars support write-position detection.
- [x] `normalize rename <target> <new-name>` — cross-file symbol rename. Uses
  `view referenced-by` to find all sites, normalize-scope for shadow/conflict detection,
  batch edit for atomic multi-file rewrite, shadow git for preview. `--dry-run` shows
  diff, no writes. This is the highest-value refactoring command.
- [x] **Refactoring engine** (`refactor/`): composable semantic actions (locate, find-references,
  check-conflicts, plan-rename/delete/insert/replace) + recipes (rename) + shared executor
  (dry-run/shadow). `do_rename` decomposed into `plan_rename` + `RefactoringExecutor::apply`.
  Foundation for move/extract/inline and future TOML-defined recipes.
- [x] **`normalize-refactor` crate extraction** — refactoring engine moved to own crate
  (`crates/normalize-refactor/`). `plan_rename` takes pre-resolved path components; caller
  does path resolution. `normalize-syntax-rules` `fix` feature gate established for future
  `PlannedEdit` integration.
- [x] `normalize move <target> <destination>` — move a symbol to another file, updating all
  import sites. Requires rename infrastructure + import rewriting. After rename lands.
  Done as `normalize edit move` (`crates/normalize-refactor/src/move_item.rs`); best-effort import rewriting for Python/Go/JS/TS; `--reexport` available.
- [ ] `normalize extract <file:start-end> <new-name>` — extract a region into a new function,
  rewriting the call site. Single-file first; cross-file as stretch.
- [x] `normalize inline <target>` — implemented as `normalize edit inline-function <file> <line>:<col>`. Single-file. See recipe at `crates/normalize-refactor/src/inline_function.rs`.
- [x] Post-edit index invalidation: after a multi-file edit, mark affected files dirty in the
  daemon's reverse-dep graph so the index refreshes without a full rebuild.
  Implemented via `Request::FilesChanged` + `DaemonClient::notify_files_changed()` called
  from `edit.rs` after every refactoring `executor.apply()`. Non-fatal if daemon is not running.

**Pillar 3 — Semantic rules (stretch goal)**

18 fact rules already exist and run via `--engine fact`. The gap is new rules and wiring
incremental evaluation so they're fast enough for pre-commit use:

- [x] Audit and remove `normalize-facts-rules-builtins` — entire crate deleted (was orphaned, not in workspace, no dependents). Datalog version runs.
- [x] New fact rules: `missing-test` (exported function with no test calling it) and
  `stale-mock` (test mock references a function that no longer exists) — both added,
  disabled by default, attribute-based detection.
- [x] New fact rule: `dead-parameter` — implemented as a **native rule** (not Datalog) using
  `normalize-scope`'s `ScopeEngine::find_unused_parameters()` since parameters are not in the
  facts schema. Requires `@local.definition.parameter` in `locals.scm`; added for Rust, Python,
  JS, TS, TSX, Go, Java, C, C++, C#. Underscore-prefixed params excluded. Default disabled.
- JIT fix and incremental eval wiring moved to 0.2.0 blockers.

**Rules engine architecture — drop abi_stable, external process + rkyv for custom rules**

The current dylib rule pack system (`libloading` + `abi_stable` + `RString`/`RVec` in
`Relations`) has a heap corruption bug (glibc "corrupted double-linked list" on
`normalize rules run --type fact`) caused by allocator boundary mismatch between the
main binary and loaded `.so` files. This is not a patch-sized fix — the design is wrong.

Target architecture:

| Rule kind | Boundary | Serialization |
|---|---|---|
| Built-in native (stale-summary, broken-ref, …) | None — compiled in | — |
| Datalog (builtin + user `.dl` files) | None — JIT in-process | — |
| Custom native Rust rules | External process | rkyv |
| Heavy external tools | External process | JSON / SARIF |

rkyv for custom native rules: the external process receives `Relations` as a zero-copy
rkyv archive (mmap or pipe), does its computation, writes diagnostics back. This gives
external-process safety (no allocator boundary, no ABI concerns) without paying full JSON
serialization cost — cheap enough for pre-commit. SARIF stays for heavy tools where JSON
overhead is acceptable.

- [x] Drop `libloading`, `abi_stable`, `loader.rs` and the dylib search-path machinery. (commit 398b715b)
- [x] Replace `RString`/`RVec` in `Relations` with plain `String`/`Vec`. Fixes heap corruption.
- [x] Add `rkyv` derive to `Relations` + fact types for the external-process boundary.
- [x] Define the external native rule protocol: receive rkyv Relations on stdin, write
  NDJSON diagnostics on stdout. Documented in `docs/rules-external-protocol.md`.

**Dependencies / preconditions:**
- `normalize refs` ships first — it's the foundation for rename, move, and dead-parameter rule.
- Incremental Datalog wiring can happen independently of new rules.
- JIT fix is upstream; don't block anything on it.
- abi_stable removal can land independently of JIT — unblock it first.

**Pillar 6 — Discoverability (every context type expressible in one call)**

The design principle: every useful type of context around a symbol or file should be
expressible as a single normalize call, not a sequence of greps and reads. This doesn't
mean every type will be *used* often — but it should *exist* as an option. Agents and
users opt in when they need it; the absence of an option forces multi-call workarounds
regardless of how rarely the context is useful.

The work is mechanical and inevitable — every context type will need to exist eventually,
so build them. Mine Claude Code session logs (adapt `scripts/session-corrections.sh` for
command-sequence analysis) to find what's *still missing* after the obvious types are
covered, not to decide what to build first.

Context types that should exist (independent of priority):
- **Blast radius**: "if I change X, what breaks?" — forward reachability (callers,
  importers, dependents). The dual: "why is X broken?" — backward reachability (what X
  depends on, its call chain). Both are index queries; neither is expressible today in
  one call without multiple `--referenced-by`/`--references`/`--graph` round-trips.
- **Directory orientation**: `normalize view <dir>` surfacing `SUMMARY.md` as a preamble
  and `//!` module docs for files. Agents get orientation + symbols without a separate
  read.
- **Change impact**: given a diff or set of changed files, what symbols/rules are affected?
  Feeds into incremental eval (Pillar 4) and debugging alike.

Concrete unblocked items:
- [x] `normalize view <directory>` surfaces `SUMMARY.md` as preamble; `--json` adds `"summary"` field.
- [x] `rust/missing-module-doc` syntax rule — `lib.rs`/`mod.rs` files with no `//!`.
- [x] Split `stale-summary` into `missing-summary` (presence) + `stale-summary` (freshness), each with `paths` glob config.
- [x] `normalize view <file>` surfaces `//!` crate/module docs and equivalents for all languages. (done — Rust `//!`, Python docstrings, Go package comments, JS JSDoc, Ruby `#` — implemented in `extract_module_doc` per language; duplicate of item at L735)

**Pillar 7 — Sub-100ms hot path (hook-grade latency)**

normalize should be fast enough to run in a hook after every single tool call — not just
pre-commit. That means the hot path (`rules run` on changed files) needs to be under 100ms,
possibly much less. Current cold `rules run` is seconds; even warm daemon routing is hundreds
of ms.

Measured (2026-03-29, normalize repo ~8450 files, ~2654 tracked):

| Component | Debug | Release | Notes |
|-----------|-------|---------|-------|
| Full `rules run` | 65s | 13s | All engines |
| Syntax only | 6s | 5.4s | 16 files matched; tree-sitter parse dominates |
| Native only | 10s | 3.3s | File walking dominates (8436 files) |
| Fact only | 30s | 13ms | Interpreter overhead in debug; release uses index |
| Single file `--only` | 57s | — | Now pre-walk scoped (was full walk, now filtered) |
| Startup (`--version`) | 13ms | — | Negligible |

Key findings:
- **`--only` now pre-walk scoped** (syntax and advisory native rules). `--rule` still filters post-walk for native engine.
- **Native cost is file walking**, not rule evaluation. 3s release just to enumerate 8k files.
- **Fact rules are free in release** (13ms) via daemon warm cache. Debug is 30s (interpreter).
- **Debug/release gap is 5x** (65s vs 13s), mostly from fact interpreter.

Projected hook budget (release, file-scoped, 1-5 changed files):
- Startup: ~13ms
- Syntax (parse 1-5 files): ~50-100ms
- Native (skip walker, check N files): ~10-50ms
- Fact (daemon): ~13ms
- **Total: ~100-200ms** — achievable

Concrete steps (ordered by impact):
- [x] **Daemon-cached diagnostics for all engines** — the daemon caches syntax, fact, and
  native rule results in `DiagnosticsCache` on `WatchedRoot`. Cache primed eagerly on
  file changes (incremental for syntax/fact, full for native) and lazily on first
  `RunRules` request. `run_rules_report()` tries daemon for all cacheable engines first
  via `try_rules_via_daemon()`. Service layer skips local native rules when
  `report.daemon_cached` is true. `RunRules` protocol extended with `engine` field.
- [x] **Pre-walk scoping for `--only`** — `PathFilter` struct in `normalize-rules-config`
  compiled from `--only`/`--exclude` globs, threaded to syntax runner (`collect_source_files`)
  and native rules (via `filtered_gitignore_walk` / `effective_files`). Post-walk filter kept
  as safety net.
- [x] **`--files` flag** — accept explicit file list, bypass walker entirely. Threaded through
  syntax runner (`run_rules`), native threshold rules (`long-file`, `high-complexity`,
  `long-function`). Fact rules unchanged (they query the index). Directory-based native
  rules (stale-summary, check-refs, etc.) still walk the tree as they are project-level checks.
- [ ] **Process overhead** — if even the daemon handoff is too slow, consider embedding normalize
  as a library in the hook process (e.g. a Claude Code hook that `dlopen`s normalize).

**Not targeting 0.3.0:**
- Full AST rewriting (tree-sitter edit API, round-trip fidelity)
- Type-aware refactoring (normalize has no type resolver)
- Jinja2 grammar crate publish

---

**Pillar 8 — Git-behavioral analysis (co-change index)**

normalize understands code *structurally* today (imports, calls, symbols). Git history is a
complementary signal encoding human intent and actual change patterns — invisible from the AST.

**The primitive: a co-change edge table in the index.**

Import and call edges are already in SQLite. Co-change edges belong there too: a
`co_change_edges(file_a, file_b, count)` table populated by `structure rebuild`, updated
incrementally. `coupling-clusters` becomes a trivial graph query instead of recomputing from
scratch. Stale-doc detection, churn analysis, and ownership queries all become free consumers.

**Why SQLite (not a separate file):** same access pattern as other edges, same invalidation
mechanism (`structure rebuild`), daemon already reads from it. No new cache invalidation logic
needed.

**Size management — per-file fanout cap (not time window):**
- **≥2 co-changes threshold**: a single co-change is coincidence; two or more is a pattern.
- **Skip large commits**: commits touching >50 files are mechanical operations (fmt, license
  headers, mass rename). They carry zero semantic signal and generate most of the noise.
- **Live files only**: prune edges where either file no longer exists in HEAD. Useless by
  definition.
- **Per-file fanout cap (K=20)**: each file stores at most its top K partners by frequency.
  This is the primary size bound — it directly targets hub files (TODO.md, Cargo.lock,
  CHANGELOG.md) that co-change with everything. Caps table size at `files × K` worst case,
  regardless of repo size or history depth. Does NOT discard old coupling that is still strong.
  Time window was considered and rejected: it's a size optimization dressed as a quality filter.
  Old strong coupling is still real coupling.

**Consumers (ordered by value):**
1. `coupling-clusters` — replace recomputation with index query (immediate win)
2. Stale-doc detection native rule — doc file + strongly-coupled code files → flag if code
   changed more recently than doc
3. Churn analysis — files with high commit frequency (already partially in `analyze hotspots`)
4. Ownership concentration — files only touched by one author

**Implementation steps:**
- [x] Add `co_change_edges` table to the index schema (`normalize-facts`) — schema v8
- [x] Populate during `structure rebuild` using gix commit walk (now PATH-independent)
- [x] Incremental update: process only commits since last rebuild (append-only, cheap)
- [x] Update `coupling-clusters` to query index instead of recomputing
- [x] Add `stale-doc` native rule as first consumer

---

**Pillar 9 — Semantic retrieval (vector embeddings over structural chunks)**

Semantic search over the codebase: embed symbols + doc comments + context windows and
query by meaning rather than name. The retrieval result is structured data — agents and
developers can locate conceptually related code without knowing exact identifiers.

**Design:** `normalize-semantic` crate (fastembed + ONNX, no server), SQLite storage
alongside the structural index, re-ranking by cosine similarity + staleness penalty.
Config: `[embeddings] enabled = true` in `.normalize/config.toml`.

**Implementation steps:**
- [x] Create `normalize-semantic` crate with embedder, chunks, store, search, populate, service modules
- [x] Add `normalize structure search <query>` command to `FactsService`
- [x] Wire population into `structure rebuild` (non-fatal, skipped when disabled)
- [x] Add `embeddings` field to `NormalizeConfig` and `RebuildReport`
- [x] Add `normalize init` CTA for semantic search
- [x] Add `assert_output_formatter::<SearchReport>()` in output.rs test
- [x] Replace heuristic `strip_doc_markers()` with tree-sitter-based extraction: `FlatSymbol.docstring` now carries clean text from `Language::extract_docstring`; stored as `doc:<text>` in `symbol_attributes`; `populate.rs` uses it directly without post-processing
- [x] Daemon incremental: `refresh_root` in daemon.rs queues a background thread to call `populate_incremental_for_paths` after each file-change refresh; non-blocking, non-fatal
- [x] Staleness computation from git history: `git_staleness.rs` walks commits per-file (cached by path); formula `min(1.0, commits_before_last_touch / 50.0)` wired into `populate_embeddings` via new `repo_root` param
- [x] ANN search via sqlite-vec: `vec_embeddings` virtual table (`vec0`) created alongside `embeddings`; `upsert_embedding` and deletes sync both tables; `run_search` tries ANN first (top-50 candidates), falls back to brute-force if extension unavailable; `SearchReport.ann_used` indicates which path was taken; extension registered per-connection via `VecConnection` (raw FFI handle with `sqlite3_vec_init` called directly, avoids `sqlite3_auto_extension` / libsql init conflict)
- [x] Embed markdown docs: `populate_markdown_docs()` embeds SUMMARY.md, CLAUDE.md, README.md, and docs/*.md chunked by heading section with breadcrumb context; wired into `structure rebuild`
- [x] Embed commit messages: `populate_commit_messages()` walks last 500 commits via gix, embeds subject+body keyed by short hash; incremental (skips already-embedded); wired into `structure rebuild`

---

## Post-polish review

After the fixpoint polish loop reaches 0 findings, do a retrospective pass:
review all the changes made during the polish loop and evaluate whether they
were actually helpful. Some fixes may have been mechanical (rename, doc comment)
with clear value; others may have introduced complexity or changed semantics in
ways worth questioning. Candidates to review: catch_unwind in FFI (does it hide
real bugs?), load_rules_config merge (was the old behavior intentional?),
find_cycles_dfs iterative conversion (was stack depth ever actually a problem?).

### `normalize sync` — project + session portability

- [x] **Single-project sync (done)**: `normalize sync <dest>` copies project tree (excludes target/, node_modules/, .git/objects/, .normalize/findings-cache.sqlite, .fastembed_cache/), session metadata, rewrites index DB paths. `--dry-run`, `--verbose`, `--all`, `--active N`, `--repo <glob>`, `--exclude <glob>`.
- [x] **Incremental sync**: `SyncManifest` records blake3 content hashes in `<dest>/.normalize/sync-manifest.json`; `copy_tree_incremental` skips unchanged files on subsequent syncs. `--force` bypasses manifest for a full re-sync. Report includes `files_unchanged` count.
- [x] **Session format detection**: `session_metadata_roots()` now delegates to `normalize_chat_sessions::project_metadata_roots()`, covering Claude Code, OpenAI Codex, Gemini CLI, and Normalize Agent via the format registry. Service layer (`service/mod.rs`) calls `project_metadata_roots` directly.

---

## Deferred

### Inferred opinionation: guess configuration / taste / consensus for free

Design: `docs/design/inferred-opinionation.md`.

Infer style conventions directly from the corpus via a per-decision-class decision
tree (features = normalize's AST/CFG/scope context; label = form chosen). Leaf
purity = strictness; residual impurity = genuine free decisions surfaced to the
decision stream. Learned tree emits directly as `normalize-rules`/`-syntax-rules`
and is enforced via `normalize-refactor`/`-edit`/`-ratchet`. Scoped to
style/formatting/control-flow only — unsafe for semantic canonicalization.

Depends on: `normalize-code-similarity` (instance bucketing), `normalize-cfg`,
`normalize-scope` (feature source), rule + refactor + edit + ratchet crates
(compile target + enforcement).

Prerequisite (first work item): infrastructure to enforce AND measure arbitrary
constraints globally, including non-numeric/categorical ones. Goal is *exponential*
collapse of the valid-program space (constrain a whole decision-class everywhere =
`m^N`), not the linear flagging today's diagnostics do. Open question: ratchet holds
numeric *counts* of non-conformance today (`f64`-only), but categorical/set-valued
global measures need a new value type on `Metric::measure_all`.

### `normalize docs` follow-ups (from 2026-05-29 multi-language landing)

- **`DocFormat`-aware doc rendering for Go/Python.** Go/Python doc bodies are currently
  emitted verbatim as `plaintext`. A renderer that parses docstring conventions (RST,
  Google-style, NumPy-style for Python; godoc conventions for Go) into cleaner Markdown
  would improve LLM-context quality. The `DocFormat` tag already exists to drive this.
- **Unify the Rust remote path onto the source-archive approach.** Rust still fetches
  remote docs via docs.rs HTML scrape (`docs_rs.rs`), while Go and Python use
  `source_archive` + `doc_tree`. Moving Rust onto the source-archive path (crates.io
  `.crate` tarball → `doc_tree`) would make all three ecosystems consistent and drop the
  HTML-to-Markdown conversion.
- **Share `escape_go_proxy` with `index/go.rs`.** (Pre-existing bug, surfaced during the
  multi-language effort.) `crates/normalize-package-index/src/index/go.rs` builds
  Go module-proxy URLs WITHOUT `!`-escaping uppercase letters in module paths/versions,
  so it 404s for modules like `github.com/Azure/...`. `go_docs.rs` added a private
  `escape_go_proxy` helper; extract it to a shared location in `index/go.rs` and use it
  there too.
- **`normalize docs` support for remaining ecosystems.** Docs currently work for Rust
  (cargo), Go, and Python. The other 9 registered ecosystems still need doc support:
  npm (JavaScript/Node), gem (Ruby), maven (Java), nuget (.NET/C#), deno
  (TypeScript/Deno), hex (Elixir), composer (PHP), conan (C/C++), nix (Nix).
  The generic dispatch infrastructure (`Ecosystem` trait, `source_archive`/`doc_tree`
  primitives) is already in place — adding a new ecosystem is mostly: implement
  `docs_extractor`/`docs_fetcher`/`package_from_symbol` for it and locate its registry
  source-archive URL. Aligned with the CLAUDE.md "maximum quality for every language"
  goal.

- `normalize jq` multi-format support (YAML/CBOR/TOML/XML via `jaq-all` with `formats` feature): currently using `jaq-core/std/json` directly to avoid `jaq-fmts` bloat. Low priority — vanilla jq is JSON-only anyway.
- `normalize rg` PCRE2 support (pcre2 feature not enabled)
- `normalize fetch`: web content retrieval for LLM context (needs design: chunking, streaming, headless browser?)
- Remaining docs: prior-art.md, hybrid-loops.md
- Memory system: `docs/design/memory.md` — SQLite-backed `store/recall/forget`. Deferred until concrete use case.
- Jinja2 grammar publish: NOT via arborium (they vendor their own); publish as our own crate (`tree-sitter-jinja2` name taken — pick another). Update normalize-grammars dep. Local `grammars/jinja2/` + `find_local_grammars()` in xtask is sufficient for now.
- view: directory output shows dir name as first line (tree style) - intentional?

## Implementation Notes

### Self-update (`normalize update`)
- Now in commands/update.rs
- GITHUB_REPO constant → "rhi-zone/normalize"
- Custom SHA256 implementation (Sha256 struct)
- Expects GitHub release with SHA256SUMS.txt

