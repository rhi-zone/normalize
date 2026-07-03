# Changelog

All notable user-facing changes to the Normalize CLI are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- **New top-level `graph` verb (CLI taxonomy inversion B2).** The dependency-graph
  commands moved out of `view` into a dedicated `graph` verb owned by `normalize-graph`:
  `normalize graph` (module/symbol/type graph analysis, was `view graph`),
  `normalize graph dependents` (was `view dependents`), and
  `normalize graph import-path` (was `view import-path`). The old `view graph` /
  `view dependents` / `view import-path` paths still work as **hidden transitional
  aliases** for one release; migrate to the `graph` verb. `normalize-graph` gained a
  `cli` feature gating the report structs, `OutputFormatter` impls, and `GraphService`
  (library consumers of the pure graph algorithms build with `default-features = false`).

### Added

- **`normalize-index` — foundational index-enabler crate.** Extracts index acquisition
  (`open`/`open_if_enabled`/`ensure_ready`/`require_import_graph`/`ensure_ready_or_warn`)
  and import-graph construction (`build_import_graph`/`ImportGraph`) out of the main crate
  and `normalize-architecture` into one leaf crate. Acquisition now takes config **slices**
  (`&IndexConfig` + `&WalkConfig`) instead of the monolithic `NormalizeConfig`, so feature
  crates can acquire the index without depending on the main crate. `IndexConfig` moved here
  (composed by `NormalizeConfig` via `#[param(nested, serde)]`). Moving `build_import_graph`
  into this shared leaf breaks the `graph ↔ architecture` dependency cycle;
  `normalize-architecture` re-exports it so its consumers are unchanged. No user-facing CLI
  change. (Unblocks the CLI taxonomy inversion B-series.)

- **`docs/crates.md` — a canonical crate registry.** A single scannable reference listing
  every workspace crate with its purpose, category, and CLI-namespace ownership. It
  replaces the removed per-directory `SUMMARY.md` convention at the crate level; each
  crate's `Cargo.toml` `description` remains the maintainable source of truth.

### Removed

- **The `SUMMARY.md` per-directory convention and its enforcing native rules
  (`missing-summary`, `stale-summary`) were removed.** The convention was
  high-friction and chronically stale, so the pre-commit hook no longer requires a
  `SUMMARY.md` in every directory, and `normalize rules run --type native` /
  `normalize ci` no longer emit `missing-summary` or `stale-summary` findings. All
  in-repo `SUMMARY.md` files were deleted. Any `[rules.rule."stale-summary"]` /
  `[rules.rule."missing-summary"]` entries in a project's `.normalize/config.toml`
  are now inert and can be removed.

### Changed

- **Internal: the `sessions` command surface moved into a new `normalize-sessions`
  crate.** The `#[cli] SessionsService`, all session report structs, and their
  `OutputFormatter` impls now live in `normalize-sessions`; the main `normalize`
  crate mounts it in one line (the "crate owns its subcommand" pattern used by
  `normalize-budget`/`normalize-cfg`). This is a pure restructure — `normalize
  sessions` behaviour, flags, and output are unchanged. The optional sessions web
  UI is still gated behind the `sessions-web` feature (now
  `normalize/sessions-web` → `normalize-sessions/sessions-web`).

- **The default build no longer bundles libsql's remote/replication stack.** normalize uses
  only local libsql databases, so the stock build now pulls `libsql` with `core`-only features.
  This drops `tonic`, `tonic-web`, `libsql_replication`, and the transitive duplicate
  `axum 0.6`/`hyper 0.14`/`h2 0.3`/`tower-http 0.4` (~40 crates) from the default dependency
  graph. Remote/replicated sqld is now opt-in via the new `remote-sqld` feature
  (`cargo build --features remote-sqld`).

### Fixed

- **Standalone `normalize-facts structure` now honors `NORMALIZE_INDEX_DIR`.** Its
  `FactsCliService` previously hardcoded `<root>/.normalize/index.sqlite`, so with
  `NORMALIZE_INDEX_DIR` set it read/wrote a different index than `normalize view graph`
  and the main `normalize structure`. It now resolves the index path via the shared
  `normalize_facts::get_normalize_dir`, so all index consumers agree on one location.

- **`normalize jq` is functional again.** Every filter previously failed with
  `compile error: undefined Filter`. This was **not** a version-mismatch problem —
  the jaq versions were already correct and coherent (jaq-core 3.1.0, jaq-std 3.0.1,
  jaq-json 2.0.1, the same set the working `--jq` path uses). The real cause: the
  vendored jq CLI was adapted from jaq 3.0.0-beta, and when jaq-core 3.1.0 moved the
  core builtins into `jaq_core::defs()`/`jaq_core::funs()`, the vendored code kept
  chaining only `jaq_std`/`jaq_json` and never registered the core builtins — so
  every filter compiled as an undefined filter. Fixed by registering
  `jaq_core::defs()`/`jaq_core::funs()` in the compiler (matching server-less-core's
  working `--jq` path); the same fix was applied to the `sessions analyze --jq` path.
  As a minor cleanup, the stale `*-beta` version pins were tidied to the release
  triple (jaq-core 3.1.0, jaq-std 3.0.1, jaq-json 2.0.1), but that was cosmetic, not
  the fix.

- **Daemon no longer fires phantom config reloads on file reads.** The daemon's
  file-watch dispatch loop treated inotify `Access(Open)` events (pure reads) on
  `.normalize/config.toml` as config changes. Because the daemon reads config.toml
  constantly while indexing and priming, its own reads triggered spurious config
  reloads (each emitting an `IndexRefreshed { files: 0 }` event and needless
  reprime churn). The dispatch loop now ignores read (`Access`) events entirely;
  real edits still arrive as `Create`/`Modify`/`Remove`.

- **`normalize init` now seeds a `[walk] exclude` section** in the generated
  `.normalize/config.toml` — the baseline (`.git/`, `.normalize/`) plus any
  auto-detected scratch dirs present in the project (e.g. `.claude/worktrees/`,
  Claude Code agent worktrees). This restores behavior that had been stranded in
  a dead-except-tests code path (`commands/init.rs::run_init`) since 2026-03-07
  and so never shipped from the live command. The dead `run_init` is removed and
  its config-seeding logic now lives in the served `init` command. `--dry-run`
  previews the seeded excludes without writing.

### Added

- **Buildable `--no-default-features` core** — `cargo build -p normalize --no-default-features`
  (no `cli`/`serve`/`daemon`) now compiles: the bare library is the reusable surface with the
  entire CLI service layer gated out. Previously this failed to compile because two grammar
  auto-install code paths in `commands/` referenced the cli-gated `service` layer unconditionally;
  they are now behind `cli`, where they belong. Library embedders can depend on the core without
  pulling the CLI, serve-transport, or daemon stacks. Guarded in CI so it can't regress.

- **`daemon` feature flag** — the background daemon **server** (multi-root file watcher +
  incremental index refresh, Unix-only) is now gated behind a `daemon` feature (`default = true`,
  so the stock binary is unchanged). The feature pulls the `notify` filesystem watcher, now
  `optional` — a build without `daemon` drops it from the dependency tree entirely. The daemon
  **client** stays always-compiled (on Unix): edit/context service flows still push change
  notifications, and with the server gated out they transparently fall back to the no-daemon
  path (identical to the daemon simply not running). `normalize daemon run` built without the
  feature prints a clear "requires the 'daemon' feature" message and exits non-zero. This
  completes the capability-surface feature pass (serve transports + daemon).

- **Serve transport feature flags** — the `normalize serve` transports are now individually
  gated capability surfaces: `lsp` (LSP/`tower-lsp`), `http` (HTTP REST + OpenAPI/`axum` +
  `utoipa`), and `mcp` (MCP/`rmcp`), with a `serve` umbrella that enables all three. All are
  `default = true` (via `serve` in the default feature set), so the stock binary ships LSP +
  HTTP and — new in this release — the **MCP server** (previously opt-in behind `mcp`). Slim
  builds can drop any transport (e.g. `--no-default-features --features cli,lsp`); the serve
  dependencies (`axum` 0.8, `utoipa`, `tower-lsp`, `rmcp`) are now `optional` and only compile
  when their transport is enabled. Invoking a transport that was compiled out prints a clear
  "requires the '<feature>' feature" message to stderr and exits non-zero.

- **`normalize-git` crate** — new standalone crate (`crates/normalize-git`) consolidating all
  pure-Rust gix read operations previously duplicated across `normalize-budget`,
  `normalize-ratchet`, `normalize-semantic`, and the main `normalize` crate. Public API:
  `open_repo`, `read_blob_text`, `read_blob_bytes`, `walk_tree_at_ref`, `diff_base_to_head`,
  `FileChangeKind`/`FileChange`, `git_head`, `git_head_branch`, `git_commit_timestamps`,
  `git_log_timestamps`, `resolve_ref`, `resolve_merge_base`, `git_show`,
  `git_diff_name_status`, `git_ls_files`, `git_remote_origin_url`,
  `git_has_uncommitted_content_changes`, `git_summary_has_uncommitted_changes`,
  `git_last_commit_for_path`, `git_commit_count_for_path`, `git_file_churn_stats`,
  `git_author_commit_counts`, `git_activity_commits`, `git_per_commit_files`,
  `format_unix_date`, `run_in_worktree`. All dependent crates migrated in the same batch (B1
  of the CLI taxonomy migration).

### Changed

- **`normalize-analyze` crate renamed to `normalize-rank`.** The crate never held analysis
  logic — it is the shared rank/render layer (`Entity` trait, `RankEntry`, `RankStats`,
  `rank_pipeline`, `format_ranked_table`, `truncate_path`). The old name oversold and
  miscategorized it. Consumers must update the dependency name (`normalize-analyze` →
  `normalize-rank`) and Rust paths (`normalize_analyze::` → `normalize_rank::`). Crate count
  unchanged (still 44 published crates).

### Fixed

- **Circular-dependency detection (`normalize view graph`) now works.** SCC computation
  (`normalize-graph::tarjan_sccs`/`find_sccs`) silently returned no clusters even for real
  cycles due to a frame-ordering bug in the iterative Tarjan: the root-check sentinel was
  pushed onto the LIFO work stack after the child frames, so it ran before children
  propagated their lowlinks and every SCC collapsed to a singleton. The sentinel is now
  pushed first (popped last), so cycles are detected correctly. Added a full SCC test suite;
  `find_bridges` was audited for the same defect and confirmed correct.

- **Import-graph commands no longer silently succeed with an empty result when the import
  graph is empty.** `view graph`, `view dependents`, `view import-path`, `rank imports`,
  `rank depth-map`, `rank layering`, and `analyze architecture` now exit non-zero with an
  actionable message (`Run \`normalize structure rebuild\` …`) when the index contains no
  import data, instead of returning a zeroed/empty report with exit 0 (a hard-constraint
  violation). The guard is centralized in `index::require_import_graph` and keys on the raw
  `imports` row count, so a populated index with a genuinely-empty *query* (e.g.
  `view import-path A B` with no path between them) still exits 0 as before.
- **Errors are now structured under `--json`/`--jsonl`/`--jq`.** Service-layer failures emit
  `{"error": "<message>"}` on stdout (exit non-zero) for programmatic consumers, instead of
  plain text on stderr. (server-less generic CLI error path.)
- **`rules show <id>` now resolves native rules.** IDs that appear in `rules list` but live in
  the native-rules registry (e.g. `stale-summary`, `missing-summary`, `ratchet/*`, `budget/*`)
  previously reported `Rule not found`; `rules show` now searches syntax, fact, **and** native
  rules — the same set `rules list` enumerates.
- **`structure packages` no longer emits an empty result.** When no package ecosystems are
  detected it now prints an explicit message (text) instead of a bare/blank line; `--json`
  continues to emit `{"ecosystems": []}`.
- **`--pretty` now works on 8 commands where it was silently inert.** `sessions stats`,
  `sessions subagents`, `analyze architecture`, `analyze cross_repo_health`, `rank files`,
  `rank size`, `rank ceremony`, and `rank contributors` advertised `--pretty` in `--help` but
  fell back to plain text — the flag never reached the renderer. They now produce the rich
  colored output (`--compact` and TTY auto-detection likewise resolve correctly, against the
  command's target root).
- **Guide bodies corrected**: 27 stale `normalize analyze <X>` examples updated to their
  current locations (`rank`, `syntax`, `view`, `trend`). The commands moved in earlier
  releases but guide text lagged. A new regression test (`guide_links.rs`) now catches
  this class of staleness automatically by resolving every `normalize <…>` example
  against the live CLI command tree.

### Added

- **`--dry-run` on mutating commands that lacked it** (CLAUDE.md hard-constraint
  remediation, audit T1-2). Each previews what would change and writes nothing:
  - `edit redo` (mirrors the existing `edit undo --dry-run`; fixes the undo/redo asymmetry).
  - `rules run --fix --dry-run` — lists every issue that would be fixed, per file, without
    applying any edits.
  - `rules add`, `rules update`, `rules remove` — preview which rule files would be
    written/deleted (downloads are still performed to compute the preview, but nothing is saved).
  - `rules setup` — walks the enable/disable decisions without writing `config.toml`.
  - `ratchet add`, `ratchet update`, `ratchet remove` — preview baseline config changes.
  - `budget add`, `budget update`, `budget remove` — preview budget config changes.
  - `kg write` — preview unit writes and deletes (including the destructive `null`-transform
    delete) without touching the knowledge graph.
  - `sessions mark`, `sessions unmark` — preview reviewed-list changes.
  - `structure rebuild` — reports the rebuild scope (full vs incremental, content types,
    root, filters, target index path) without opening or writing `.normalize/index.sqlite`.
  - The dry-run state is reflected in the typed report (`dry_run` field) for programmatic
    consumers, not just the text output.

### Changed

- **Adopted server-less 0.6 (CLI capability-wiring invariant).** Global `--pretty`/`--compact`
  flags are now delivered through a single `CliGlobals` sink per service instead of per-method
  parameters, removing a class of silently-inert flags. `normalize-ratchet`, `normalize-budget`,
  and `package` no longer carry a private `--pretty` advertisement (they have no distinct pretty
  output); `--pretty` remains available as a root-level global. Subcommand `--help` now lists
  `--manual` and shows `[possible values: …]` for enum flags (server-less 0.6).

- **`normalize rank` text output is converging on one house style** (documented in
  `docs/cli-design.md`, "Rank output house style"). First wave of the migration:
  - `rank complexity` now renders a single auto-width table with a `Risk` column
    (Low/Moderate/High/Critical) instead of `### Critical` / `### High Risk`
    subsections, and folds its summary stats into the `#` title
    (`# Complexity — 30 functions, avg 2.4, max 9, 0 critical, 0 high`) rather than a
    key-value preamble block. The risk-band thresholds moved to `--help`.
  - `rank ownership` and `rank coupling` dropped their trailing footnotes; the
    bus-factor and confidence-formula explanations moved into each command's `--help`.
    Column headers are now spelled out (`Bus Factor` not `BF`, `Authors` not `Auth`,
    `Shared Commits` not `Shared`, `Confidence` not `Conf%`), and the Top Author
    column is no longer mid-string truncated.
- **`normalize rank` house-style migration — wave 2:**
  - `rank length` now renders an auto-width table with `Lines`, `Risk`, and `Function`
    columns (dropping `### Too Long`/`### Long`/`### Medium` subsections and the
    `N lines` unit suffix inside values). The `Risk` column uses shared `RiskTier`
    vocabulary (Low/Moderate/High/Critical). Summary stats are inline in the `#` title
    (`# Function Length — 56 functions, avg 13.5, max 172, 2 too long, 1 long`).
  - `rank test-gaps` now renders an auto-width table with a `Risk` column and spelled-out
    headers (`Risk Score`, `Risk`, `Function`, `Location`, `Complexity`, `Callers`, `Lines`).
    The hand-rolled fixed-width columns and 24/36-char path truncation are gone. Title
    follows the `# Name — stat, stat` spec.
  - `rank test-ratio` title fixed to the standard `# Test/Impl Ratio — stat, stat` shape
    (was `# Test/Impl Ratio: path — …`). Diff mode now uses `# Test/Impl Ratio Diff vs
    <ref>` prefix. A `format_pretty()` implementation was added.
  - `rank imports` column headers corrected to title-case: `Fan-In` (was `Fan-in`),
    `Imported Names` (was `Imported names`).
  - `rank files`'s `## By Language` section is now rendered via `format_ranked_table`
    (`Language`, `Lines` columns) instead of a hand-rolled `N lines  Lang` format.
    `format_pretty()` was added.
- **`normalize rank` house-style migration — wave 2, batch C:** five hand-rolled-table
  offenders converted to `RankEntry` + `format_ranked_table`:
  - `rank module-health`: preamble key-value block (`Root:`, `Modules scored:`) folded
    into the `#` title. Table migrated from hand-rolled fixed-width columns to
    `format_ranked_table`; `RankEntry` implemented on `ModuleHealthEntry`. Column headers
    title-cased and spelled out (`Score`, `Test`, `Uniqueness`, `Density`, `Ceremony`,
    `Logic`, `Lines`, `Module`). Two-space row indentation dropped. `format_pretty()`
    now uses `pretty_ranked_table` with score-based tier coloring.
  - `rank call-complexity`: three hand-rolled sections (`Top Amplified`, `Highest
    Reachable CC`, `By Module`) converted to `format_ranked_table` calls. Abbreviated
    headers expanded (`amplif` → `Amplification`, `reach#` → `Reachable Count`,
    `avg_amp` → `Avg Amplification`, `max_reach` → `Max Reachable CC`). The `x` suffix
    moved from value cells to the `Amplification` / `Avg Amplification` column values
    (kept as unit suffix on the value string, matching convention). Summary stats folded
    into the `#` title. `format_pretty()` added via `pretty_ranked_table` with
    amplification-based tier coloring on the Top Amplified section. Formula explanations
    moved to `--help`.
  - `rank ceremony`: key-value preamble block (`Interface impl methods`, `Inherent/class
    methods`, `Free functions`) folded into the `#` title as inline stats. `## By
    Language` section migrated from a hand-rolled three-column format to
    `format_ranked_table` with `Ratio`, `Interface Impl`, `Total`, `Language` columns.
    `format_pretty()` added via `pretty_ranked_table`.
  - `rank hotspots`: title now `#`-prefixed. Table converted from hand-rolled fixed-width
    columns with 48-char path truncation to `format_ranked_table` (auto-width, no
    truncation). Footer formula footnotes moved to `--help`. `format_pretty()` added.
  - `rank contributors`: all three sections (`Author Summary`, `Repo Summary`, `Author
    Overlap`) given `#` titles and converted from hand-rolled fixed-width tables to
    `format_ranked_table`. `BF` abbreviation expanded to `Bus Factor`. `format_pretty()`
    added via `pretty_ranked_table`. Bus Factor explanation moved to `--help`.
- **`normalize rank` house-style migration — wave 2, batch B:** raw `\x1b[...]` escape
  sequences replaced with `nu_ansi_term` throughout four commands:
  - `rank surface` `format_pretty()` replaced with `pretty_ranked_table`.
  - `rank depth-map` `format_pretty()` replaced with `pretty_ranked_table`.
  - `rank layering` `format_pretty()` replaced with `pretty_ranked_table`; abbreviated
    column headers expanded to spelled-out title-case (`Down` → `Downward`,
    `Up` → `Upward`, `Self` → `Same Layer`).
  - `rank density` preamble key-value block (`Root:`, `Files analyzed:`,
    `Compression ratio:`, `Token uniqueness:`) folded into the `#` title as inline
    stats. Precision inconsistency fixed: both overall stats and table columns now use
    `.3` decimal places consistently (was `.2` in the preamble, `.3` in the table).
    `format_pretty()` replaced with `pretty_ranked_table`. Metric explanations moved
    to `--help`.
- **`normalize rank` house-style migration — wave 2, batch D (final):** remaining 6
  subcommands brought to spec:
  - `rank budget`: numbers are now bare integers throughout (no thousands commas, no `K`
    suffix on module line counts). Title changed to `# Line Budget — N lines, root`.
    `format_pretty()` replaced: category table rendered via `pretty_ranked_table` with
    per-category colors, followed by a bar-chart distribution section.
  - `rank duplicate-types`: `#` title added with inline stats (`files scanned`, `types`,
    `pairs`, `min overlap`); the key-value preamble block is gone. `To suppress:` command
    hint removed from the body (guidance belongs in `--help`).
  - `rank size`: title changed to `# Code Size — N lines, root` (no trailing `\n`/extra
    blank line). `format_pretty()` added (bold title + tree body via `nu_ansi_term`).
    Tree body kept as-is (hierarchical, not a table).
  - `rank duplicates`: `#` title added with inline stats (`groups/pairs`, `files scanned`,
    `items analyzed`, `threshold` when applicable). `format_pretty()` title updated to
    match.
  - `rank uniqueness`: key-value preamble (`Root:`, `Files analyzed:`, `Functions
    analyzed:`, etc.) folded into the `#` title. `format_pretty()` now uses
    `pretty_ranked_table` for the modules section with uniqueness-ratio coloring.
  - `rank fragments`: raw `\x1b[...]` escape codes replaced with `nu_ansi_term`
    throughout `format_pretty()`. Column headers spelled out in title-case (`Total Lines`
    not `TotalLn`, `Avg Lines` not `AvgLn`). Title changed to `# Fragment Analysis —
    stat, stat, …` format. Fixed hardcoded column widths in `format_text()`.
- **`normalize rank` house-style migration — wave 3 (post-audit fixes):** eight remaining
  deviations identified by the 2026-06-16 independent audit are now resolved:
  - `rank hotspots` title now includes inline stats (`# Git Hotspots — N files, top score
    S, max churn C`); the parenthetical descriptor `(high churn)` is dropped (context
    belongs in `--help`). The recency variant retains a brief qualifier in the command
    name: `# Git Hotspots (recency-weighted) — …`.
  - `rank coupling` title now includes inline stats (`# Temporal Coupling — N pairs, max
    confidence P%`); the parenthetical `(files that change together)` is gone.
  - `rank ownership` title now includes inline stats (`# File Ownership — N files, avg bus
    factor F, N single-author`); the parenthetical `(git blame)` is gone.
  - `rank contributors` gains an outer command-level title with aggregate stats
    (`# Contributors — N authors, N repos, N commits`) ahead of the sub-section tables;
    the three sub-section headers are demoted from `#` to `##`.
  - `rank call-complexity` column headers: `Local CC` → `Local Complexity`, `Reachable CC`
    → `Reachable Complexity`, `Max Reachable CC` → `Max Reachable Complexity` (CC was
    unexplained in output; spelling out avoids the ambiguity).
  - `rank uniqueness` column header: `Fns` → `Functions`.
  - `rank ceremony` per-file column header: `Impl` → `Implementations`.
  - `rank density` column header: `Compress` → `Compression`.
  - `rank test-ratio` title word and column header: `Impl` → `Implementation`; full title
    is now `# Test/Implementation Ratio — …`.
  - `rank duplicates` suppression lines no longer embed CLI flag guidance (`use
    --include-trait-impls to show`); the guidance moved to the `--help` doc comment. The
    factual `Suppressed: N …` counts remain in the output body.

- **`budget measure` and `budget add` flag `--base-ref` renamed to `--diff-ref`.**
  The old name was inherited from an earlier draft; `--diff-ref` correctly describes
  the flag — it specifies the git ref to diff against when measuring line/function
  growth. Behaviour is unchanged; only the flag name changed.

### Fixed

- **`trend complexity/length/density/test-ratio` commands now work when historical
  commits contain a stale `[embeddings]` config section.** The config loader
  previously called `process::exit(1)` on any config containing the removed
  `[embeddings]` section. Spawned git worktrees created by `trend` commands check
  out historical `.normalize/config.toml` files that may still have this section;
  the hard abort caused all four metric-trend commands to fail entirely.
  The check is now a recoverable warning — the section is silently ignored by serde
  since it no longer appears in `NormalizeConfig` — and the command continues.

- **Usage strings no longer show `normalize.elf` in the installed release binary.**
  `main.rs` now rewrites `argv[0]` to its `file_stem()` before passing to clap, so
  usage lines always print `normalize` regardless of the on-disk binary name (which is
  `normalize.elf` in the musl-loader release layout).
- **CommonJS `require()` imports and re-exports now extracted via `.scm` queries
  for JavaScript, TypeScript, and TSX.** Previously JS/TS/TSX bypassed the
  language-agnostic `.scm` path via a hand-rolled AST walker; the walker handled
  imports, exports, and re-exports but only for those three languages. All patterns
  are now in `javascript.imports.scm`, `typescript.imports.scm`, and
  `tsx.imports.scm`: ES6 `import` statements, CommonJS `require()` (simple binding,
  shorthand destructured, aliased destructured, bare side-effect), and `export …
  from` re-exports (named, wildcard `*`, namespace `* as ns`). TSX now also
  extracts re-exports (it previously lacked those patterns). No behavior change for
  ES6 imports; `require()` and re-export extraction now also benefit from the same
  `.scm`-driven improvements applied to other languages.
- **`syntax query` top-level alternation `[...]` now returns correct matches.**
  Queries whose entire pattern is a top-level tree-sitter alternation (e.g.
  `[(identifier) @i (line_comment) @c]`) previously returned 0 matches silently
  because `is_sexp_pattern()` only checked for a leading `(`, causing the query
  to be mis-dispatched to ast-grep which silently no-ops on S-expression input.
  `is_sexp_pattern()` now also recognises a leading `[` as an S-expression pattern.
  Nested alternations (e.g. `(call_expression [(identifier) @a (string) @b])`) were
  unaffected and continue to work.
- **Daemon native-rules refresh no longer builds an unbounded backlog.** The
  daemon watched each repo's `.git/index` and pushed the root into an unbounded
  channel on every change; under heavy git churn the producer (~5/s) outran the
  consumer (~0.9/s), building a backlog (~57k deep in one observed case) that
  pegged ~2 cores for hours after activity stopped. Because each native refresh
  re-reads the root's current on-disk state, repeated refreshes of the same root
  are redundant. The channel is replaced by a per-root coalescing set ("latest
  wins") so the backlog can never exceed the number of watched roots regardless
  of churn — coalescing is exact (identical final index), not lossy.
- **Daemon spin detector now catches `.git/index`-driven runaways.** The
  existing detector only fired when a refresh's changed-set overlapped the root's
  own `.normalize/` state dir, so a `.git/index`-driven backlog (paths outside
  `.normalize/`) never tripped it. A second signal flags a root when native-rules
  refresh density is high *and* the coalescing queue is not draining, and the
  dispatch loop now applies per-root spin backoff to both the full-refresh and
  native-refresh paths.
- **Daemon diagnostics writes no longer fail with SQLite errors.** Fixed
  `table daemon_diagnostics has no column named issues_blob` (the diagnostics
  tables are now self-healed at index open if a stale column shape is detected)
  and `cannot start a transaction within a transaction` (every `BEGIN…COMMIT`
  block in the structural index now rolls back on error and clears any leaked
  transaction before starting a new one, instead of leaving the reused connection
  wedged after a mid-transaction failure).
- **`normalize daemon stop` now removes the socket and lock files.** Graceful
  shutdown previously left `~/.config/normalize/daemon.lock` and the socket on
  disk; both are now removed before exit. (The OS already released the advisory
  flock on process exit, and startup tolerates a stale lock file from a crashed
  daemon — this is a cleanliness fix.)

### Changed

- **`normalize docs` bodies are now source-native with a `doc_format` tag.**
  The doc body is stored verbatim in `doc_body` alongside a `doc_format`
  (`markdown` / `rst` / `html` / `plaintext`); rendering to display Markdown
  moved to the output layer instead of being precomputed at fetch time, so
  `--json` consumers receive the raw body and choose their own rendering. Rust
  `///` docs are `markdown`, docs.rs docblocks are `html` (converted on
  display), and Go/Python docstrings are `plaintext`. The knowledge-graph cache
  key prefix changed from `docs-cargo-` to `docs-rust-…` (language-derived),
  invalidating previously-cached doc units — they regenerate on next fetch.

- **`normalize kg` collapsed to 3 primitives: `read`, `write`, `walk`.** The 11-verb surface (create/get/set/append/delete/link/unlink/edges/query/neighbors/show) is replaced by three indivisible operations. `write` accepts a jq transform (null = delete, object = put/patch); `read` accepts an id or `-q` jq predicate; `walk` does BFS traversal via a jq expression that extracts link target IDs. Embedded jq via the `jaq` crate — no external jq dependency. `--depth N` and `--include-start` on `walk`.

- **Move graph-substrate design and thesis docs into the kg knowledge graph** (units: `graph-substrate-design`, `graph-substrate-thesis`). Originals removed from `docs/design/` and `docs/introspection/`. Linked by a `derived-from` edge; anchor metadata preserves prior paths. See `normalize kg read graph-substrate-design` and `normalize kg read graph-substrate-thesis`.

- **`normalize kg` edge storage moved from shared log to per-unit frontmatter.** Outgoing edges are now stored in each source unit's `links` YAML frontmatter field (`[{kind, to, metadata?}]`) instead of a shared append-only `edges.jsonl` log. Per-unit ownership makes branches that add edges to different units merge cleanly. On first use, any existing `edges.jsonl` is auto-migrated and renamed to `edges.jsonl.migrated-v0`.

### Added

- **Refactoring engine (extract-function / add-parameter / inline-variable /
  introduce-variable) now supports more languages.** In addition to Rust, Python,
  JavaScript, TypeScript, and TSX, the following languages gained both a
  `<lang>.refactor.scm` structural-classification query and a `RefactorCodeGen`
  source-synthesis implementation: Go, Java, Ruby, Lua, Kotlin, Swift, Dart, C#,
  PHP, Scala, Groovy, Visual Basic, Zig, D. (Swift add-parameter and Dart
  add-parameter call-site rewriting are limited by their grammars, and VB
  reassignment detection is limited by grammar ambiguity; see TODO.md.)

- **`normalize docs` now supports Go and Python**, in addition to Rust. Symbol
  documentation is resolved from local source first (Go module cache /
  `$GOMODCACHE`, Python venv `site-packages`) and, for uninstalled packages,
  from the upstream package source archive — the Go module proxy
  (`{module}/@v/{version}.zip`) and the PyPI sdist — rather than scraping a docs
  site. Symbol syntax is ecosystem-specific: Rust `crate::path::Sym`, Go
  `import/path#Sym` or `pkg.Sym` (e.g. `fmt.Println`), Python `pkg.Sym`.

- **`--ecosystem`/`-e` flag on `normalize docs`** to select or disambiguate the
  ecosystem. When omitted, the ecosystem is auto-detected from the working
  directory; the command errors clearly when none or more than one docs-capable
  ecosystem is present.

- **Daemon logs to a file and self-detects spin loops.** Auto-started daemons previously ran
  with stdout/stderr connected to `/dev/null`, so all `tracing` output — including the runaway
  re-index "spin" that burned CPU for hours — was silently discarded. The daemon now routes its
  logs to `~/.config/normalize/daemon.log` (same dir as the socket/lock; honors
  `NORMALIZE_DAEMON_CONFIG_DIR`) when auto-started; foreground `daemon run` still logs to the
  terminal. In addition, the daemon now watches for the spin signal directly: when a refresh's
  changed-file set overlaps the root's own index/state directory at high density (≥5 refreshes in
  10s), it flags the root, backs off refreshes for that root for 30s (per-root, never global),
  emits a WARN, and records a structured spin warning surfaced by `normalize daemon status`.
  Indexing is never silently dropped — backoff plus a loud, visible warning only.

- **`normalize kg` knowledge graph** — new `normalize-knowledge-graph` crate with v0 primitives: unit CRUD (`create`/`get`/`set`/`append`/`delete`), edge management (`link`/`unlink`/`edges`), and query/traversal (`query`/`neighbors`/`show`). Filesystem-backed in `.normalize/kg/` with YAML frontmatter units and per-unit `links` arrays. Dotted-path metadata matching (e.g. `--match anchors.symbol=Frobnicator`). BFS neighbor traversal at configurable depth.

### Fixed

- **Daemon no longer spins on projects with a `config.toml` that omits `[walk]`.** When
  a project has a `.normalize/config.toml` with no `[walk]` section, the walker previously
  resolved to an empty `WalkConfig` (bypassing the bootstrap defaults), descended into
  `.normalize/`, mutated `index.sqlite`, and looped forever at ~22% CPU. Fixed by adding
  `WalkConfig::with_daemon_baseline()` — called in all three daemon walk-config load paths
  (`normalize/src/index.rs`, `normalize-facts/src/service.rs`,
  `normalize-rules/src/runner.rs`) — which unconditionally ensures `.git/` and
  `.normalize/` are in the exclusion list regardless of config file presence.

- **`normalize kg write/walk` jq expressions now use `.metadata.links` consistently.**
  Previously, jq transforms saw `links` as a top-level field (separate from `metadata`)
  so `.metadata.links += [...]` would silently discard changes and `.metadata.links[].to`
  would fail with a null-iteration error on units with no links. The jq-facing JSON
  representation now always embeds `links` inside `metadata` as an array (empty when no
  links exist), matching the on-disk YAML frontmatter format.

- **`normalize kg` build: tree-sitter removed from flake devShell `buildInputs`.**
  `tree-sitter` was in `buildInputs`, causing NixOS to add its include path to
  `NIX_CFLAGS_COMPILE` which then leaked into `rust-lld`'s input list as a bare path,
  producing "cannot open .../include: Is a directory" linker errors. Moved to PATH
  via `shellHook` (same treatment as `musl.dev` in the prior fix).

- **`normalize sessions ... --all-projects` now honors `CLAUDE_SESSIONS_DIR`.** The
  `list_all_project_dirs` helper previously hardcoded `~/.claude/projects`, so
  `--all-projects` ignored the env var that the rest of the session machinery
  respects. `LogFormat` now exposes a `projects_root()` method (defaulting to
  `None`); `ClaudeCodeFormat` overrides it to return `$CLAUDE_SESSIONS_DIR` when set.

## [0.3.2] - 2026-05-10

### Fixed

- **musl release binary now runs on NixOS and non-FHS distros.** The bundled
  `runtime/libgcc_s.so.1` was previously Ubuntu's glibc-linked copy, which
  depends on `ld-linux-x86-64.so.2` — absent on NixOS. The release workflow
  now sources `libgcc_s.so.1` from Alpine Linux (a musl-based distro), so the
  library depends on `libc.so` (musl) instead. The zig/cargo-zigbuild approach
  introduced to work around this is reverted; the build uses `musl-gcc` again.

### Changed

- **`normalize context --help` now shows comprehensive inline reference.** The help output
  includes frontmatter format, `--match` dot-path syntax, `--stdin`/`--prefix` JSON injection,
  `--file` structured file loading, and examples. Previously the description was a one-liner.

### Fixed

- **`normalize context --help` no longer shows a duplicate `context` subcommand.**
  The default action method was named `context` (same as the parent service), causing
  `normalize context context` to appear in help output. The method is now hidden from
  the subcommand list (`#[cli(hidden)]`); `normalize context` continues to work as
  the default action with all flags hoisted to the parent command.

- **`cargo xtask build-grammars --cc "zig cc -target x86_64-linux-musl"` now works.**
  The `--cc` argument is split on whitespace so compound compilers like `zig cc -target
  x86_64-linux-musl` are correctly parsed into program + arguments. Previously `Command::new`
  was called with the entire string as the binary name, causing "No such file or directory"
  for every grammar. zig's lld linker also requires `--allow-shlib-undefined` instead of
  `--unresolved-symbols=ignore-in-shared-libs`; the xtask now detects zig cc and emits the
  correct flag.

- **Grammar ABI mismatch after `normalize update`.** `ensure_grammars_first_use` now
  reads the `.installed-version` stamp and compares it against the running binary's
  version. If they differ (e.g. after a self-update), the stamp is deleted and grammars
  are re-downloaded for the current binary before any command runs. Previously, an
  existing stamp caused the check to short-circuit unconditionally, leaving stale 0.2.x
  `.so` files loaded by a 0.3.x binary.
- **`normalize update` now invalidates the grammar stamp** immediately after replacing
  the binary, so the next invocation triggers a grammar re-download even if the process
  exits before `ensure_grammars_first_use` runs.
- **Friendly error for removed `[embeddings]` config key.** Loading `.normalize/config.toml`
  now pre-checks for `[embeddings]` (removed in 0.3.0) and exits with a clear migration
  message instead of a generic parse error.

### Added

- **`normalize edit extract-function <file> --lines <start>-<end> --name <name> [--apply]` command.** Extracts a line range from a function into a new function using CFG liveness analysis. Infers parameters (variables live into the region from outside) and return values (variables defined inside the region and live after it) via backward-dataflow fixed-point over the facts index. Checks `cfg_effects` for async, generator, defer, and acquire/release semantics; emits warnings for defer crossing boundary, unbalanced resource lifetime, and escaping exception edges. Generates language-appropriate source for Rust, Python, Go, TypeScript/JavaScript, and Java. Default is dry-run; `--apply` writes the changes. Requires `normalize structure rebuild`.

- **CFG Phase 4: type-refined exception flow.** `Edge` now carries `exception_type: Option<String>` for `EdgeKind::Exception` edges (None = conservative/unknown; `Some("T")` = typed). The CFG builder captures `@cfg.exit.throw.type` and `@cfg.try.catch.type` from `.cfg.scm` queries to emit typed exception edges. Exception edges in `build_try` are emitted per catch type; `ExitThrow` edges carry the thrown type.
- **Exception type captures in 5 languages.** Java: thrown type from `object_creation_expression.type`; catch type from `catch_formal_parameter/catch_type/type_identifier` (handles multi-catch `IOException | SQLException`). Python: thrown type from `raise_statement/call.function`; catch type from `except_clause/identifier` (single) and `except_clause/as_pattern/tuple/identifier` (multi). JavaScript/TypeScript/TSX: thrown type from `throw_statement/new_expression.constructor`; catch clauses are untyped (catches all). C++: thrown type from `throw_statement/call_expression.function` (`identifier` or `qualified_identifier`); catch type from `catch_clause/parameter_list/parameter_declaration.type`. C#: thrown type from `throw_statement/object_creation_expression.type`; catch type from `catch_clause/catch_declaration.type`.
- **`exception_type` column in `cfg_edges` SQLite table.** Nullable TEXT column added to `cfg_edges`. Schema version bumped to 15. Both `refresh_call_graph` and `reindex_files` paths updated.
- **`CfgEdgeFact.exception_type` field.** `cfg_edge` Datalog preamble relation extended to 7 fields: `cfg_edge(file, func, func_line, from, to, kind, exception_type)`. `liveness.dl` updated to use the 7-field form (`_, _` for the two new wildcards). `relations.add_cfg_edge` gains the `exception_type` parameter. Both `facts.rs` and `runner.rs` now load `all_cfg_edges()` from the index into `Relations`.
- **`exception_flow.dl` builtin Datalog rule.** Derives `exception_reaches(file, func, func_line, throw_block, catch_block, type)`, `unhandled_exception(file, func, func_line, throw_block, type)`, and `can_throw(file, func, func_line)`. Disabled by default; enable with `normalize rules enable exception_flow`.
- **Mermaid renderer shows exception type on edges.** Exception edges now render as `b3 -->|"exception: IOException"| b5` when a type is known, and `b3 -->|"exception"| exit` when conservative.
- **`normalize analyze exceptions <file> [--function <name>]` command.** Reports throw sites with their exception type and the catch clauses they route to. Flags unhandled throws (escaping to function exit). Shows catch clauses with types and handled-throw counts (including "dead catch?" annotation for clauses that handle 0 throws). Requires `normalize structure rebuild`.
- **`Cfg::throw_edges()` helper.** Returns an iterator over all `EdgeKind::Exception` edges in the CFG.

- **CFG Phase 3: effects tracking.** `BasicBlock` now carries `effects: Vec<Effect>`. New `EffectKind` enum: `Await`, `Defer`, `Yield`, `Acquire`, `Release`, `Send`, `Receive`. New `BlockKind` variants: `Deferred`, `Acquire`, `Release`. New `EdgeKind` variants: `Suspend`, `Resume`. The builder collects `@cfg.effect.*` captures from `.cfg.scm` queries and assigns them to the enclosing block.
- **Effect queries for Rust, Python, TypeScript, JavaScript, Go.** `@cfg.effect.await` on `await_expression` (Rust, TS, JS) and `(await)` (Python). `@cfg.effect.yield` on `yield_expression` (TS, JS) and `(yield)` (Python). `@cfg.effect.acquire` on `with_statement` (Python). Go effect queries: `@cfg.effect.defer` on `defer_statement`, `@cfg.effect.send` on `go_statement` and `send_statement`, `@cfg.effect.receive` on unary `<-` expressions.
- **`cfg_effects` SQLite table.** New table in the structural index: `cfg_effects (file, function_qname, function_start_line, block_id, kind, byte_offset, line, label)`. Populated by `normalize structure rebuild`. Schema version bumped to 14.
- **`CfgEffectFact` Datalog relation.** New `cfg_effect(file, func, func_line, block, kind, line, label)` relation in `normalize-facts-rules-api`, exposed in the Datalog preamble and loaded in the `rules run` and `facts` pipelines.
- **`effects.dl` builtin rule.** Derives `async_function`, `defer_function`, `generator_function`, `resource_acquire`, `resource_leak` from `cfg_effect`. Disabled by default; enable with `normalize rules enable effects`.
- **`normalize analyze effects <file> [--function <name>]` command.** Reports suspension points, deferred calls, yields, resource acquisitions, and channel operations for functions in a file. Requires `normalize structure rebuild`.

- **`normalize analyze liveness <file> --function <name>` command.** Computes live-in and live-out variable sets per basic block using standard backward-dataflow liveness analysis. Requires the structural index (`normalize structure rebuild`). Returns a `LivenessReport` with per-block `BlockLiveness` entries showing which variables are live at block entry and exit.
- **CFG Phase 2: def/use captures.** `BasicBlock` now carries `defs: Vec<DefSite>` and `uses: Vec<UseSite>`. The builder recognises `@cfg.def`/`@cfg.def.name` and `@cfg.use`/`@cfg.use.name` captures from `.cfg.scm` queries and assigns them to the enclosing block. Rust, Python, and Go `.cfg.scm` files updated with variable definition captures.
- **CFG SQLite persistence.** Four new tables in the structural index: `cfg_blocks`, `cfg_edges`, `cfg_defs`, `cfg_uses`. Populated by `normalize structure rebuild` for all files whose language has a `.cfg.scm` query. Schema version bumped to 13.
- **Datalog CFG relations.** Four new relations in `normalize-facts-rules-api`: `cfg_block(file, func, func_line, block, kind)`, `cfg_edge(file, func, func_line, from, to, kind)`, `cfg_def(file, func, func_line, block, name)`, `cfg_use(file, func, func_line, block, name)`. Available in `.dl` rule files via the preamble.
- **`liveness.dl` builtin rule.** Registers standard backward-dataflow liveness (`live_in`/`live_out` derived relations) as a disabled-by-default built-in Datalog rule. Enable with `normalize rules enable liveness`.

- **`normalize cfg` command.** New `normalize-cfg` crate with a control flow graph builder and Mermaid renderer. `normalize cfg <file> [-f <function>]` builds a CFG from any file with a supported `.cfg.scm` query and renders it as a Mermaid `flowchart TD`. Data model: `Cfg`, `BasicBlock` (with `BlockKind`: Entry, Exit, Statement, Branch, LoopHead, LoopBody, LoopExit, Catch, Unreachable), `Edge` (with `EdgeKind`: Fallthrough, ConditionalTrue, ConditionalFalse, BackEdge, Break, Continue, Return, Exception). Rust, Python, and Go have bundled queries.
- **`GrammarLoader::get_cfg(name)`** in `normalize-languages`. Parallel to `get_complexity`, loads `.cfg.scm` query files from the grammar search path with fallback to bundled queries.
- **Rust CFG query (`rust.cfg.scm`).** Captures `if_expression` (branch + condition/then/else), `match_expression` (match + arms), `while_expression`/`for_expression` (loop + condition/body), `loop_expression` (unconditional loop + body), `return_expression`, `break_expression`, `continue_expression`, and `panic!`/`todo!`/`unreachable!` macros (throw). Snapshot tests for 6 fixtures: linear, branch, loop, nested, early_return, match.
- **Python CFG query (`python.cfg.scm`).** Captures `if_statement` (branch + condition/then/else), `match_statement`/`case_clause` (Python 3.10+), `for_statement`/`while_statement` (loop + condition/body), `try_statement`/`except_clause`/`finally_clause`, `return_statement`, `break_statement`, `continue_statement`, `raise_statement`. Snapshot tests for 4 fixtures: linear, branch, loop, early_return.
- **Go CFG query (`go.cfg.scm`).** Captures `if_statement` (branch + then/else), `expression_switch_statement`/`expression_case_clause` (match), `for_statement` (loop + body; covers range, condition, and unconditional forms), `return_statement`, `break_statement`, `continue_statement`. Snapshot tests for 4 fixtures (skipped gracefully when Go grammar not installed).
- **TypeScript and TSX CFG queries (`typescript.cfg.scm`, `tsx.cfg.scm`).** Captures `if_statement`/`else_clause` (branch), `switch_statement`/`switch_case`/`switch_default` (match), `for_statement`/`for_in_statement`/`while_statement`/`do_statement` (loop), `try_statement`/`catch_clause`/`finally_clause`, `return_statement`, `break_statement`, `continue_statement`, `throw_statement`. Verified against arborium-typescript and arborium-tsx node-types.json. Snapshot tests for 6 fixtures: linear, branch, loop, early_return, try_catch, switch.
- **JavaScript CFG query (`javascript.cfg.scm`).** Identical control flow grammar as TypeScript (shared arborium base). Same captures; verified against arborium-javascript node-types.json. Snapshot tests for 4 fixtures: linear, branch, loop, early_return.
- **Java CFG query (`java.cfg.scm`).** Captures `if_statement` (branch), `switch_expression`/`switch_block_statement_group`/`switch_rule` (match — covers both statement and expression form), `for_statement`/`enhanced_for_statement`/`while_statement`/`do_statement` (loop), `try_statement`/`try_with_resources_statement`/`catch_clause`/`finally_clause`, `return_statement`, `break_statement`, `continue_statement`, `throw_statement`. Labeled break/continue are captured as exits; label resolution deferred. Verified against arborium-java node-types.json. Snapshot tests for 5 fixtures (skipped gracefully when Java grammar not installed).
- **CFG coverage matrix test.** New `coverage_matrix.rs` test in `normalize-cfg/tests/` classifies all registered languages as `HAS_CFG` (rust, python, go, typescript, tsx, javascript, java), `NOT_APPLICABLE` (data/markup/config formats: json, yaml, toml, xml, html, css, scss, graphql, sql, and others), or `DEFERRED` (languages with control flow but no query yet). The `cfg_has_cfg_languages_return_some` test asserts all HAS_CFG grammar names return `Some` from `GrammarLoader::get_cfg`.

- **CFG Phase 1: batch queries for 69 additional languages.** Added `.cfg.scm` query files for all DEFERRED languages: C-family (C, C++, ObjC, C#, Kotlin, Swift, Dart), JVM/functional (Scala, Groovy, VB, Haskell, OCaml, F#, Elixir, Erlang, Clojure, Gleam, ReScript, Idris, Agda, Lean, CommonLisp, Scheme, Elisp), scripting (Ruby, Lua, PHP, Perl, Bash, Fish, Awk, Zsh, PowerShell, Batch, Vim), systems/scientific (Zig, Ada, D, Prolog, R, Julia, MATLAB, GLSL, HLSL, Verilog, VHDL), and domain/config (Nix, HCL, Starlark, Elm, Jinja2, Svelte, Vue, CMake, Meson, TLA+, jq). Moved `dockerfile` and `query` (tree-sitter query language) to NOT_APPLICABLE. Remaining DEFERRED: asm, x86asm (assembly — need grammar inspection), uiua (array language). Snapshot tests added for Lua and Jinja2 (grammars installed); all other tests skip gracefully when grammar not installed. Coverage matrix `cfg_has_cfg_languages_return_some` now verifies 76 languages.

- **`ModuleResolver` for 7 additional languages.** Elm (`elm.json` source-directories, module name → file path), Nix (relative `./path.nix` resolution; `<nixpkgs>` → NotFound), R (`source("./file.R")` relative load; `library(pkg)` → NotFound), Julia (`include("file.jl")` relative include + workspace `Project.toml` package lookup), MATLAB (filename stem = function name; searches workspace root + `src/` + `lib/`), Prolog (relative `use_module` + bare name search in workspace root; `library(...)` → NotFound), D (`dub.json` sourcePaths, `mypackage.utils` → `mypackage/utils.d`). Resolver matrix test updated.

- **`ModuleResolver` for 20 additional languages.** JVM languages: Java, Kotlin, Groovy, Scala (Maven/Gradle `src/main/<lang>` conventions). .NET languages: C#, VB, F# (namespace→file path mapping). Swift (SPM `Sources/<target>` directory targets). Dart (pubspec.yaml `package:` import resolution). Zig (`@import` relative path resolution). Elixir (Mix `lib/` CamelCase↔snake_case). Erlang (1:1 module=file). Haskell (Cabal `hs-source-dirs`). OCaml (capitalized stem convention). Lua (`require` dot-path). PHP (composer.json PSR-4 autoload). Perl (`lib/` `::` path). Clojure (`src/` dot-namespace). Common Lisp (workspace stem). Scheme (R7RS `.sld`/`.scm`). Gleam (`gleam.toml` src/). ReScript (bsconfig.json sources).

- **Phase 0 scaffold: cross-file name resolution infrastructure.** New Datalog predicates in `normalize-facts-rules-api`: `resolved_import`, `module`, `export`, `reexport`, `symbol_use`, `resolved_reference`, `resolved_call`, `module_search_path`. New `ModuleResolver` trait in `normalize-languages::traits` for per-language import resolution, with supporting types `ImportSpec`, `ModuleId`, `Resolution`, `ResolverConfig`. New crate `normalize-module-resolve` re-exporting the trait and types. New `resolution.dl` Datalog rules deriving `resolved_reference` and `resolved_call` (disabled by default, requires `normalize structure rebuild`).
- **Rust `ModuleResolver` (`RustModuleResolver`).** Resolves `use`/`mod` import specifiers to file paths within Cargo workspaces. Handles `workspace_config` (parses `Cargo.toml` for workspace members and crate names), `module_of_file` (derives canonical module path from file's position under `src/`), and `resolve` (maps `crate::module::name` to the `.rs` file, handles `super::`/`self::` relative paths, returns `NotFound` for stdlib and external crates). Tested with a 3-file fixture in `normalize-refactor/tests/`.
- **TypeScript/TSX `ModuleResolver` (`TsModuleResolver`).** Resolves relative imports (`./`, `../`), tsconfig.json `compilerOptions.paths` aliases and `baseUrl`, and `.js`→`.ts` extension elision. Returns `NotApplicable` for non-TS/TSX files, `NotFound` for node_modules.
- **JavaScript `ModuleResolver` (`JsModuleResolver`).** Resolves relative imports (`.js`, `.mjs`, `/index.js`), jsconfig.json `compilerOptions.paths` and `baseUrl`. Returns `NotFound` for bare specifiers (node_modules).
- **Python `ModuleResolver` (`PythonModuleResolver`).** Resolves relative imports (`from . import`, `from ..pkg import`), detects `src/` layout, searches workspace root for absolute imports. Returns `NotFound` for stdlib/third-party.
- **Go `ModuleResolver` (`GoModuleResolver`).** Parses `go.mod` to extract the module path; resolves import paths to directory targets within the module. Returns `NotFound` for stdlib and third-party packages.
- **Ruby `ModuleResolver` (`RubyModuleResolver`).** Resolves `require_relative` to `.rb` files relative to the caller. Returns `NotFound` for bare `require` (gems not modeled).
- **`ModuleResolver` pass in `structure rebuild` pipeline.** After the existing `resolve_all_imports` pass, `resolve_imports_via_module_resolver()` runs as a second pass using per-language resolvers to populate `resolved_file` for any imports still unresolved. Applies to full rebuild, incremental update, and single-file `update_file`. All three rebuild paths now include this pass.
- **`find_references` confidence tagging.** `CallerRef` and `ImportRef` now carry a `confidence` field (`"resolved"` | `"heuristic"`). Results are tagged `"resolved"` when the definition file's language has a `ModuleResolver`, and `"heuristic"` otherwise. Downstream consumers (rename, safe-delete) can filter on this field.

## [0.3.1] — 2026-05-08

### Removed

- **Semantic embedding search dropped from the shipping binary.** `normalize structure search`, `normalize context --semantic`, the `[embeddings]` config section, daemon incremental re-embedding, markdown/commit/context-block embedding during `structure rebuild`, the `embeddings` cargo feature flag, and the `semantic_compat` shim are all removed from the `normalize` binary. The `normalize-semantic` crate remains published on crates.io for standalone use; symbol search is being redesigned around discrete tags for a future release. Release builds simplify to a single `cargo build` step (musl no longer needs a no-default-features carve-out).

### Added

- **First-run grammar install.** The first time a user runs any
  `normalize` subcommand other than `grammars`/`--help`/`--version`,
  `normalize` checks whether tree-sitter grammars are installed in the
  user's config directory (`~/.config/normalize/grammars/`). If not, and
  the session is non-interactive (piped or driven by an agent), grammars
  are downloaded and installed automatically with a stderr notice. In an
  interactive session the check is deferred to `normalize init`, which
  prompts implicitly by installing on first invocation. Subsequent
  invocations short-circuit on a `.installed-version` stamp file, so the
  check has zero cost after first use. Users who already have a populated
  grammar directory (e.g. from `cargo xtask build-grammars` or
  `NORMALIZE_GRAMMAR_PATH`) are detected and stamped without a download.
- **`normalize structure test-fixtures`** — language extraction fixture runner. Discovers `<lang>/<case>/input.<ext>` + `expected.json` pairs under `crates/normalize-languages/tests/fixtures/`, extracts symbols/imports/calls via `SymbolParser`, and diffs against expected JSON. Flags: `--lang <lang>` (filter to one language), `--fixture-dir <dir>` (custom fixture root), `--update` (write actual output as expected — bootstrap mode). Schema: `{exhaustive, symbols: [{name, kind, line}], imports: [{module, name, line}], calls: [{callee, line}]}`. All fields optional; subset matching by default; `"exhaustive": true` for exhaustive checking.
- **`normalize rules test-fixtures`** — fixture-based test format for native/syntax rules. Discovers test cases with input source + expected diagnostics under `.normalize/rule-tests/` (configurable via `--fixture-dir`) and runs them through the engine with subset or exhaustive matching.
- **`normalize view chunk <file> --chunk N` and `--around <pattern>`** — large-file navigation for agents. `--chunk N` divides a file into fixed-size chunks (default 100 lines, configurable via `--chunk-size`) and shows chunk N (1-indexed). `--around <pattern>` finds the first regex/substring match and shows ±50 lines of context (configurable via `--context-lines`). Multiple matches: `--match-index I` to navigate. Returns `ChunkedViewReport` with `file`, `chunk`, `total_chunks`, `line_start`, `line_end`, `content`, and (for `--around`) `match_line`, `total_matches`, `match_index`. Works with `--json`/`--jq`/`--jsonl`.
- **`normalize view import-path <from> <to>`** — find the shortest import chain between two files via BFS over the resolved import graph (requires `normalize structure rebuild`). `--all` returns all simple paths up to `--limit` (default 5); `--reverse` finds paths from `<to>` to `<from>`. Emits `file → file → file` chains or "No import path found between A and B" when unreachable.
- **`normalize edit add-parameter`** — add a parameter to a function signature and update all call sites. `normalize edit add-parameter <file> <function> --param <name> --default <value> [--type <type>] [--position <N>] [--dry-run]`. Uses tree-sitter to locate the function and argument lists; finds callers via the facts index (falls back gracefully when unavailable). Supports Rust, TypeScript/JavaScript, Python.
- **`normalize edit inline-function <file> <line>:<col>`** — inline a single-use function at its call site within the same file. Locates the function definition (or resolves the name from a call-site position), substitutes arguments for parameters using whole-word replacement, strips the `return` keyword when the call is in expression position, replaces the call with the inlined body, and removes the now-dead definition. Supports JS/TS function declarations and arrow `const` bindings, Python `def`, and Rust `fn`. Conservative: aborts if the function has multiple `return` statements or if the argument count doesn't match. `--force` to inline a function with multiple call sites (only the first is inlined).
- **`normalize edit introduce-variable <file> <range> <name>`** — extract an expression at the given `line:col-line:col` range into a named binding inserted before the containing statement. Supports Rust (`let`), JS/TS (`const`), Python (bare assignment).
- **`normalize edit inline-variable`** — inverse of `introduce-variable`: replace all uses of a variable with its initializer and delete the binding.
- **Post-edit index invalidation for refactoring commands.** After any refactoring edit (`rename`, `introduce-variable`, `inline-variable`, `add-parameter`, `move`, `inline-function`) writes files to disk, the command immediately notifies the daemon via the new `FilesChanged` request. The daemon broadcasts `FileChanged` events for each path and triggers an incremental index refresh — no need to wait for inotify or run `normalize structure rebuild` manually. Non-fatal: silently skipped when the daemon is not running.
- **`normalize sync`** — copy a project (and its AI agent session metadata) to a destination for portability. Excludes `target/`, `node_modules/`, `.git/objects/`, `.normalize/findings-cache.sqlite` by default. After copying, rewrites absolute paths in the index DB so the copy works from its new location. Supports `--dry-run`, `--verbose`, `--all` (sync all known projects), `--active N` (only projects with activity in the last N days), `--repo <glob>`, `--exclude <glob>`. Subsequent runs are incremental via a manifest of previously synced files.
- **`normalize sessions parallelization [session-id]`** — finds turns with sequential same-type tool calls that could be parallelized (e.g. `Read(foo.rs) → Read(bar.rs)`). `--threshold N` controls minimum group size (default: 2). Works per-session or aggregated across filtered sessions.
- **`normalize sessions heatmap [session-id]`** — per-file read/write counts across a session. Classifies files as `hot` (>5 writes), `read_only` (reads with 0 writes — potential test gap), or `normal`. Sorted by write count, `--top N` to limit rows.
- **`normalize sessions cost [session-id]`** — per-turn token cost breakdown using model-specific Anthropic pricing. Shows input/output/cache-read/cache-write tokens and estimated USD per turn. Summary includes total cost, cost without cache, cache savings (USD), and cache efficiency %.
- **Composable `sessions messages` filters and review state.** `sessions messages` now accepts `--has-tool`, `--errors-only`, `--exclude-interrupted`, `--turn-range`, `--min-chars`, `--max-chars`. New `sessions mark`/`sessions unmark` subcommands write/remove session IDs from `.normalize/sessions-reviewed`; `sessions list` accepts `--reviewed`/`--unreviewed` to filter accordingly.
- **`high-fan-out` and `high-fan-in` native rules** — coupling smell detection. `high-fan-out` flags files that import from more than `threshold` (default: 20) distinct resolved modules; `high-fan-in` flags files imported by more than `threshold` distinct files. Both are default disabled, require `normalize structure rebuild`, and are configurable via `[rules.rule."high-fan-out"] threshold = N` / `[rules.rule."high-fan-in"] threshold = N`. Tagged `architecture` and `coupling`.
- **`boundary-violations` native rule** — configurable directory-level import boundary enforcement. Declare pairs like `"services/ cannot import cli/"` under `[rules.rule."boundary-violations"] boundaries = [...]`; the rule queries the structural index's `imports` table and reports each resolved import that crosses a boundary. Default disabled; requires `normalize structure rebuild`. Uses glob matching with `services/` treated as `services/**`.
- **`dead-parameter` native rule** — flags function parameters that are never referenced in the function body. Uses `normalize-scope`'s `ScopeEngine` with `@local.definition.parameter` captures in `locals.scm`. Underscore-prefixed names (`_x`) are excluded. Supported: Rust, Python, JavaScript, TypeScript, TSX, Go, Java, C, C++, C#. Default disabled.
- **Re-export tracing in import resolution.** `pub use path::Item` in Rust and `export { X } from './y'` in TypeScript/JavaScript are now extracted as re-exports (`is_reexport = 1` in the `imports` table). After `resolve_all_imports()`, a new `trace_reexports()` pass follows re-export chains (up to 10 hops, with cycle detection) so imports in file A that land on an intermediate re-exporter B are updated to point to B's source file C. This improves call graph accuracy and `find_callers`/`find_callees` results across module boundaries. Schema bumped v11→v12.
- **`#match?`/`#eq?` predicate evaluation for tree-sitter queries.** `normalize_languages::satisfies_predicates` now evaluates the standard predicates (`#match?`, `#not-match?`, `#eq?`, `#not-eq?`) so `.scm` query authors can filter captures by text content or equality. Unknown predicates continue to pass (forward-compatible).
- **`normalize-surface-syntax` pattern matching / destructuring as first-class IR nodes** — `Pat` enum (`Ident`, `Object(Vec<PatField>)`, `Array(Vec<Option<Pat>>, Option<String>)`, `Rest`) and `PatField { key, pat, default }` added to the IR; `Stmt::Destructure { pat, value, mutable }` replaces the old lowering of `const { a, b } = obj` to multiple `Stmt::Let` bindings. TypeScript reader produces `Stmt::Destructure` with full structural fidelity (shorthand, renamed, nested, defaults, object rest, array holes, array rest); TypeScript writer emits `const { a, b: c } = obj` and `const [x, y, ...rest] = arr`. Python reader maps `a, b = f()`, `[x, y] = arr`, `(a, *rest) = items`. Lua writer lowers to `local a, b = table.unpack(expr)`.
- **`normalize-surface-syntax` type annotations and template literals.** `Param` struct replaces `String` for function parameters and carries `type_annotation: Option<String>`; `Function` gains `return_type`; `Stmt::Let` gains `type_annotation`. TypeScript and Python readers populate these fields; writers emit them. `Expr::TemplateLiteral(Vec<TemplatePart>)` added for backtick strings; TypeScript writer emits backtick syntax, Python writer emits f-strings, Lua writer lowers to `..` concatenation.
- **`normalize-surface-syntax` import/export and class definitions in IR.** Imports and exports are now first-class IR nodes (previously elided). Class definitions carry methods, fields, and inheritance.
- **`normalize-surface-syntax` Lua reader improvements** — `obj:method(args)` desugars to `obj.method(obj, args)` with implicit self. Table constructor parsing fixes: `["string"] = value` computed string keys properly extract the string value; multi-variable generic for (`for k, v in pairs(t)`) preserves all loop variables; numeric for reads the optional step field. Fixed elseif chaining bug. Object key emission uses idiomatic bare identifier syntax for valid Lua identifiers. String escaping handles null bytes.
- **`normalize-surface-syntax` JavaScript reader and TypeScript reader gap fixes.** A dedicated JavaScript reader joins the existing TypeScript reader; remaining gaps in the TypeScript reader (comments preservation, additional grammar nodes) are filled.
- **`normalize-typegen` IR improvements** — `Field` gains `nullable: bool`, `default: Option<DefaultValue>`, and `constraints: Option<FieldConstraints>` (min/max, min/max_length, pattern, format). `Schema::validate()` checks for well-formedness: valid identifiers, no duplicate type/field names, unresolved `Ref` targets, and circular reference detection via DFS.
- **`normalize-typegen` JSON Schema, GraphQL SDL, and Protobuf output backends** (`backend-jsonschema`, `backend-graphql`, `backend-proto` features). JSON Schema draft 2020-12 with `$defs`/`$ref`, `anyOf`/`oneOf`, `required`, `additionalProperties: false`. GraphQL SDL: structs → `type`/`input`, string enums → UPPER_CASE variants, tagged unions → `union`. Protobuf proto3: structs → `message`, string enums with mandatory `_UNSPECIFIED = 0`, tagged unions → `oneof`, arrays → `repeated`, optional fields → `optional` keyword.
- **`normalize-typegen` Protobuf and GraphQL SDL input parsers** — round-trip support for both formats; combined with the writers above, schemas can flow between any pair of supported representations.
- **`normalize-typegen` `--split` and `--dry-run` CLI; IR source locations.** Schemas can be emitted as one file per type via `--split`. IR carries source locations for error reporting.
- **Daemon-cached diagnostics for all engines.** The daemon caches syntax, fact, and native rule diagnostics and serves them instantly on `normalize rules run`. Cache is primed eagerly on file changes (incremental for syntax/fact, full re-run for native) and lazily on first request. Falls back to local evaluation transparently when the daemon is unavailable.
- **Daemon push of diagnostic deltas via binary subscribe.** New `Event::DiagnosticsUpdated { root, updates }` variant carries per-file deltas (only files whose issues changed since the last refresh; empty `issues` Vec = file became clean). Subscribe connections opened with the `0x01` magic byte stream events as length-prefixed rkyv binary frames; the JSON-line subscribe path is unchanged for backward compatibility. New `DaemonClient::watch_events_binary` decodes the binary frames. Eliminates the LSP poll-after-`IndexRefreshed` pattern. Wire-schema change: `Event` path fields are now `String` (previously `PathBuf`) so the enum is rkyv-serializable.
- **Per-file diagnostic storage in the daemon + JSON mirror.** Schema bump v9→v10 adds a `daemon_diagnostics_per_file` table (path PRIMARY KEY, rkyv `Vec<Issue>` blob) populated on every prime/refresh. Per-file pulls (e.g. `normalize rules run path/to/file.rs`) hit this table directly via a new `filter_files` field on `RunRules` instead of fetching and filtering the whole "all" blob. The daemon also writes `.normalize/diagnostics.json` atomically on every prime/refresh as a canonical-state artifact for ephemeral consumers.
- **Daemon live-reloads `.normalize/config.toml` and `.normalize/rules/**` on change.** Previously the daemon's notify watcher saw config edits but routed them nowhere, so cached `RunRules` results stayed under the old config until either a source file changed or the daemon was restarted. The dispatch loop now classifies config and rule-definition edits as a fourth route; the handler clears every cached diagnostic blob and triggers a reprime against the freshly-loaded `NormalizeConfig`. `[daemon]` startup keys (`enabled`, `auto_start`) still require a restart by design.
- **Cross-daemon-restart cache validity (`config_hash` gate).** The SQLite-backed daemon diagnostic blobs (`daemon_diagnostics`, `daemon_diagnostics_per_file`) carry the hash of the inputs that produced them (binary version + `.normalize/config.toml` + `.normalize/rules/**`). On load, a hash mismatch is treated as a cache miss; the daemon reprimes under the current config. Schema bump v10→v11.
- **Tier 1 daemon config reload — filter-only changes apply at serve, no reprime.** When `.normalize/config.toml` edits affect only severity, allow-lists, or `enabled = false`, the daemon updates its cached config snapshot, sets a `serve_filter_pending` flag, and lets the next `RunRules` re-filter the cached findings in place. No blob clear, no rule re-evaluation.
- **Tier 2 daemon config reload — per-rule surgical re-eval.** When config edits affect only specific rules (a rule is newly enabled, or its `extra` threshold/field changed), the daemon re-runs only those rules through the syntax, fact, and native engines using the existing `filter_ids` path, splices updated findings into the per-engine blobs, rebuilds the "all" blob and per-file rows, and broadcasts a `DiagnosticsUpdated` event. `ConfigDiff::rules_to_rerun` names the affected rule IDs; `surgical_rerun_rules` implements the splice. `[walk] exclude` changes still trigger a full reprime.
- **`ConfigDiff` for surgical daemon cache invalidation.** `normalize-rules-config` exports `ConfigDiff::compute(old, new)` which classifies a config change as filter-only, per-rule re-run, or full reprime. Consumed by the daemon tiers above.
- **Daemon `.scm` file hash tracking.** Custom rule edits (`.normalize/rules/**/*.scm`) participate in the Tier 2 surgical re-eval path via content-hash tracking, not just mtime.
- **LSP native rule diagnostics.** The LSP server publishes diagnostics from native rules (`missing-summary`, `stale-summary`, `check-refs`, etc.) alongside syntax and fact rule diagnostics. Native rules run debounced and workspace-wide. They re-trigger on `.git/index` changes so `git add` events immediately refresh stale-summary results.
- **No `git` binary required.** All git operations now use `gix` (pure-Rust gitoxide): `git blame` (ownership, provenance, `view history`), `git status --porcelain`, path-filtered commit counts, `git rev-list --count`, budget metrics diff, ratchet ref-based check/measure. A `git` binary in `$PATH` is no longer a runtime dependency.
- **Configurable walker exclusions** — new `[walk]` section in `.normalize/config.toml` controls directory walking. `ignore_files` configures which gitignore-format files are respected (default: `[".gitignore"]`; `[]` to disable). `exclude` accepts gitignore-style globs (default: `[".git", ".claude/worktrees/"]`). Threaded through native rules, syntax rules, the unified rules runner, the daemon, and the LSP server.
- **Co-change edge index** — `normalize structure rebuild` populates a `co_change_edges` table in the SQLite index with file pairs that frequently change together (co-change count ≥ 2, commits touching >50 files skipped as noise, per-file fanout capped at top 20 partners). Incremental: only new commits since the last rebuild are processed. `normalize analyze coupling-clusters` queries this table instead of re-walking git history; falls back transparently to the git walk when the table is empty. Rebuild output now includes a `co_change_edges` count.
- **`stale-doc` native rule** — flags documentation files that are likely stale because strongly co-changed code files have been updated more recently. Queries the `co_change_edges` index for each doc file (`**/*.md`, `**/*.rst`, `docs/**/*`), finds code files it historically changes with, and flags the doc if any partner was committed more recently. `SUMMARY.md` is excluded (covered by `stale-summary`). Configurable via `[rules.rule."stale-doc"]` with `min_co_changes` (default 3), `min_lag_days` (default 0), and `doc_patterns`. Default disabled; requires `normalize structure rebuild`.
- **`missing-test` fact rule** — flags public functions that are never called from a test function (a function with a test attribute such as `#[test]`, `@test`, `@Test`, or `@pytest.mark`). Default disabled. Entry-point and module-boundary files excluded via the default allow list.
- **`stale-mock` fact rule** — flags mock/stub functions (identified by attributes such as `@Mock`, `@patch`, `@stub`, `mock`, `stub`, `fake`) that call a callee which no longer exists as a symbol in the index. Catches mocks that were not updated after a rename or deletion. Default disabled.
- **`normalize edit move`** — moves a symbol's definition to another file and rewrites import statements in every file that imported it from the old location. Per-language module-path derivation is best-effort: Python, Go, and JavaScript/TypeScript imports are rewritten when a new path can be derived; Rust and unsupported cases emit warnings and skip the import site rather than fabricating wrong paths. `--reexport` (Python only) leaves a re-export stub at the source location. Supports `--dry-run` and shadow-history `--message`. Leading decorations (doc comments, attributes, decorators, annotations, pragmas) preceding the symbol are included in the move, classified by tree-sitter `node.kind()` rather than text patterns.
- **`normalize config validate` deep validation.** Runs four phases (TOML syntax, JSON Schema compliance, serde deserialization, rules config parsing) on both project and global config files. Reports errors with file path, line/column when available, and validation phase. Exits non-zero on errors for CI/hook use.
- **`normalize grep <path>`** — optional positional `path` argument scopes the search tree (consistent with `view`, `edit`, `rank`). The existing `--root` flag is preserved for backward compatibility; `path` takes precedence when both are given.
- **`normalize rules run --files`** — accept an explicit list of file paths, bypassing the file tree walker entirely. Critical for hook-grade latency where the caller already knows which files changed. Composes with `--only`/`--exclude` for further filtering.
- **`normalize rules run --only`/`--exclude`** — glob pattern filtering. `--only "*.rs"` restricts to Rust files; `--exclude "tests/"` skips test directories. Applied pre-walk for syntax rules and advisory native rules; post-walk for fact rules.
- **`normalize structure rebuild --only`/`--exclude`** — glob pattern filtering for which files get indexed. Files not matching the filter are removed from the index after the walk.
- **`normalize analyze architecture --limit`** — caps the number of `cross_imports` entries in the output (default 20, `--limit 0` disables). Reduces default JSON response from ~196KB to ~10KB.
- **Command aliases** — `search`/`find` → `grep`, `lint` → `rules run`, `check` → `ci`, `index` → `structure rebuild`, `refactor` → `edit`. Users from other tools find familiar names work transparently.
- **Tiered help output** — `normalize --help` groups commands into four sections (Core, Analysis, Utilities, Infrastructure) instead of a flat alphabetical list. Core commands (view, grep, edit, rules, structure, init) appear first.
- **Daemon-cache timing diagnostic** — `normalize rules run` prints `[timings] daemon-cache: ...` to stderr when diagnostics are served from the daemon's pre-warmed cache, making it visible that the fast path was taken.
- **`NORMALIZE_DAEMON_CONFIG_DIR` test override** — when set, this env var redirects the daemon's `daemon.sock`, `daemon.lock`, and `daemon-spawn.lock` to the named directory. Lets integration tests spawn isolated daemons without contending with the user's running instance. Production behavior is unchanged when unset.
- **`normalize trend`** — top-level subcommand for time-series health metrics. Replaces `normalize analyze complexity-trend`, `analyze length-trend`, `analyze density-trend`, `analyze test-ratio-trend`, and `analyze trend`. New names: `normalize trend complexity`, `normalize trend length`, `normalize trend density`, `normalize trend test-ratio`, `normalize trend multi`.
- **`normalize package tree --depth N`** — caps the dependency tree at depth `N` (0 = roots only). Limits both text and JSON output. Default: unlimited.

### Changed

- **All tree-sitter grammars now load uniformly via `dlopen()`.** Previously
  `normalize-surface-syntax` and `normalize-typegen` statically linked four
  `arborium-*` grammar crates (TypeScript, JavaScript, Lua, Python — and
  GraphQL in typegen), while every other grammar loaded dynamically through
  the shared `GrammarLoader`. The static linking is gone: those readers now
  request grammars from the process-wide `grammar_loader()` singleton like
  the rest of the codebase. Net effect on the binary: the four (five for
  typegen) compiled-in parsers are removed; the runtime requirement that
  the relevant `.so` files live in a search path was already true for all
  other languages and is unchanged. Cargo features `read-typescript`,
  `read-javascript`, `read-lua`, `read-python`, `input-typescript`, and
  `input-graphql` continue to work — they now gate the reader source code,
  not a grammar dependency.
- **musl release build is now fully self-contained — no system runtime dependencies.** Previously the `x86_64-unknown-linux-musl` artifact was a static-pie binary (couldn't `dlopen()` grammar `.so` files), then briefly a dynamic binary that required the system to provide `ld-musl-x86_64.so.1` and `libc.musl-x86_64.so.1`. The release tarball now bundles its own musl loader and libc alongside a tiny POSIX-sh wrapper script that invokes the bundled loader explicitly (`exec "$DIR/runtime/ld-musl-x86_64.so.1" --library-path "$DIR/runtime" "$DIR/runtime/normalize.elf" "$@"`). The artifact runs on any Linux x86_64 system — Alpine, distroless, NixOS without `pkgs.musl`, glibc-only distros — with no installed musl required. `install.sh` extracts the tarball under `~/.local/share/normalize/` and symlinks the wrapper into `~/.local/bin/`. The musl artifact is now also the safe default on systems without glibc (and the only choice on NixOS).
- **rusqlite → libsql migration across the workspace.** `normalize-facts` (CA cache), `normalize-native-rules` (findings cache), `normalize-syntax-rules` (findings cache), and the `normalize sync` path-rewrite all moved off `rusqlite` onto `libsql`. Resolves a sqlite link conflict that previously forced workspace-wide symbol coordination. No user-visible behavior change; cache files at `~/.config/normalize/ca-cache.sqlite` and `.normalize/findings-cache.sqlite` are still SQLite and remain compatible.
- **`*-allow` files removed in favor of `config.toml` entries.** The 7 legacy `.normalize/*-allow` files (`large-files-allow`, `hotspots-allow`, `duplicate-blocks-allow`, `duplicate-functions-allow`, `duplicate-types-allow`, `similar-blocks-allow`, `similar-functions-allow`) are no longer loaded. Their entries now live directly in `config.toml`: `large-files-allow` → `[rules.rule."long-file"] allow = [...]`; `hotspots-allow` → `[analyze] hotspots_exclude`; duplicate/similar command allowlists → `[analyze.<subcommand>] allow = [...]`. **Migration:** if you have custom entries in any `*-allow` file, move them to the appropriate `config.toml` section.
- **Per-rule config moved to `[rules.rule."<id>"]`.** Previously `[rules]` hosted both engine-wide bare keys (e.g. `global-allow`, `sarif-tools`) and per-rule sub-tables (e.g. `[rules."rust/dbg-macro"]`) — a TOML namespace collision waiting to happen. Per-rule overrides are now nested under a dedicated `rule` sub-table; the bare-key namespace under `[rules]` is reserved for engine-wide configuration. The legacy layout is still parsed for one release with a stderr deprecation warning. **Migration:** rename every `[rules."<id>"]` to `[rules.rule."<id>"]`. Engine-wide keys stay where they are.
- **`[walk] exclude` now accepts gitignore-style glob patterns.** Previously each entry was matched only against directory entry basenames. Patterns are now compiled via a gitignore matcher anchored at the project root, so any pattern that works in `.gitignore` works here. Existing configs (e.g. `[".git", "worktrees"]`) keep working unchanged because gitignore patterns without slashes still match at any depth.
- **`normalize init --setup` detects scratch directories.** When `.claude/worktrees/` is present, the bootstrap step adds it to `[walk] exclude` by default. The `Default::default()` for `NormalizeConfig` is now genuinely empty; the opinionated bootstrap config lives separately and is only applied during `init`.
- **`normalize rules run --only`/`--exclude` pre-walk scoping.** Glob patterns are applied *before* file parsing and walking, not just after. Single-file `--only` runs are now proportional to the matched file count, not the full tree.
- **`normalize rules run` routes through daemon when running.** If `normalize daemon start` is active, `normalize rules run` (and any invocation that hits fact rules) sends the request to the daemon via Unix socket and receives pre-warmed Datalog evaluation results instead of cold-evaluating from scratch (~45 seconds on large codebases). Falls back transparently when no daemon is running.
- **`normalize structure rebuild`** defaults to incremental mode (mtime-based). Only files changed since the last build are re-indexed. Pass `--full` to force a complete rebuild. When no files have changed, the command prints "Index up to date". The `--json` output includes an `incremental: true` field when incremental mode was used.
- **`normalize view --dir-context`** accepts an integer `N` instead of a boolean flag. `N` selects context files using Python `list[:N]` semantics on the target→root ordered list: `1` = target dir only, `2` = target + parent, `-1` = all ancestors, `0` = none.
- **`normalize view --dir-context` JSON output** includes a `dir_context` field in `ViewReport` containing the merged context content. Previously the context was only prepended to text output; agents using `--json` received no context.
- **`normalize rules tags`** always populates the `rules` array in JSON output. The `--show-rules` flag has been removed.
- **`normalize syntax ast`** default depth changed from unlimited (`-1`) to `5`. Pass `--depth -1` to restore the old unlimited behavior.
- **`normalize analyze docs --json`** `by_language` field serializes as named objects `{"documented": N, "total": N}` instead of positional arrays.
- **`normalize grammars list --json`** returns objects with `name` and `path` fields instead of bare strings. Text output is unchanged.
- **`normalize analyze architecture` compact output** no longer truncates hub and symbol paths with opaque worktree-hash prefixes. Paths are shown as clean workspace-relative paths.
- **`normalize context` compact output** includes `<!-- source -->` file path comments and `---` separators between blocks when multiple context files are merged. Single-block output is unchanged.
- **`normalize ci` / `normalize rules run` compact output** `(N files)` header now reads `(N files checked)` to clarify it is the number of files scanned, not files with issues.
- **`normalize grep`** consecutive matches within the same symbol are grouped under a single `(SymbolName L48-61):` header rather than repeating the symbol tag on every line.
- **`normalize view <file>:N-M`** header no longer duplicates the line range (was `file.rs:10-20:10-20`, now `file.rs:10-20`).
- **`normalize analyze length` → `normalize rank length`**; **`normalize analyze test-gaps` → `normalize rank test-gaps`** — ranking commands moved under `rank`.
- **`normalize analyze node-types` removed** — duplicate of `normalize syntax node-types`. Use the latter.
- **`large-file` rule renamed to `long-file`** — consistency with `long-function`. Update `[rules.rule."large-file"]` to `[rules.rule."long-file"]` in your config.
- **`long-file`, `high-complexity`, `long-function` available as native rules.** Threshold-based health findings from `analyze health` are now usable in `normalize rules run --type native`. Default disabled (advisory); enable via `[rules.rule."long-file"] enabled = true` or `--rule long-file`. Defaults: 500 lines, complexity 20, 100 lines. Thresholds configurable via `threshold` key.
- **`FileRule` trait for native file-based rules** — `long-function`, `high-complexity`, and `long-file` implement a `FileRule` trait providing automatic SQLite caching and parallel execution. New file-based rules get caching for free by implementing `check_file()` and `to_diagnostics()`.
- **`validate-calls-scm` moved out of SARIF tools** — the internal `.calls.scm` capture-name validation is no longer a `[[rules.sarif-tools]]` entry. It is now a direct step in `scripts/pre-commit` using a single `grep` invocation (~5 ms vs ~762 ms).
- **Mtime-based cache for SARIF tools** — `[[rules.sarif-tools]]` entries support an optional `watch` field (list of glob patterns). When set, `normalize rules run` caches the tool's output in the SQLite findings cache keyed by the max mtime of all matching files, skipping the tool on warm runs where nothing changed.

### Performance

- **Persistent symbol cache for single-file commands.** `Extractor` checks the CA cache (`~/.config/normalize/ca-cache.sqlite`) before running tree-sitter on a file. On cache hit (same blake3 content hash, grammar, and `include_private` setting), the stored `Vec<Symbol>` is returned immediately — no parse, no query execution. Cross-file resolver paths are excluded since their results depend on other files.
- **Persistent tree-sitter query cache for facts extraction.** Tree-sitter query results are cached in SQLite keyed by content hash and grammar version; `normalize structure rebuild` reuses extraction output for unchanged files instead of re-running queries.
- **`normalize context` v2 daemon caching.** Context queries hit the daemon cache for near-zero latency on warm runs.
- **rkyv binary IPC for daemon rules cache.** `normalize rules run` communicates with the daemon using a zero-copy binary protocol instead of three JSON round-trips: magic byte (`0x01`), 5-byte binary frame header (`[type_byte][4-byte LE len]`), rkyv-serialized payload. The daemon pre-builds an "all" blob on every refresh so the common unfiltered case is a direct blob read. Schema bump v8→v9. Round-trip: ~6–8 ms vs ~82 ms previously.
- **Parallel fact rule evaluation.** `run_rules_batch` evaluates all enabled Datalog rules in parallel using rayon. On a typical 8-core machine with 7 enabled rules, `normalize rules run --type fact` drops from ~5.5 s to ~2.5 s wall time. JIT is not used in the parallel path (the ascent-interpreter JIT internals are not thread-safe under concurrent engine initialization); the sequential JIT path is still used for the incremental/daemon-cached path via `run_rule_with_cache`.
- **Syntax and native rules findings cache: WAL mode + single transaction.** The per-file SQLite findings cache used by syntax rules and native rules opens with `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;` and wraps cache-write operations in a single `BEGIN`/`COMMIT` transaction per run. Cold run: ~1.6 s (vs ~19 s before); warm run: ~0.84 s total.
- **SQLite findings cache for native and syntax rules.** Warm runs of `long-file`, `high-complexity`, and `long-function` skip unchanged files entirely using a SQLite-backed per-file cache stored at `.normalize/findings-cache.sqlite`. The syntax rules engine migrates from the JSON `syntax-cache.json` to the same SQLite store. Cache keys include `(path, mtime_nanos, config_hash, engine)`; changing the threshold or rule set invalidates only the affected entries.
- **Batched uncommitted-changes check for stale/missing-summary.** `stale-summary` and `missing-summary` open the git repository once and collect all changed paths into a `HashSet` before the directory loop, replacing hundreds of per-directory gix calls. Warm pre-commit runs drop from ~2.2 s per rule to ~170 ms.
- **Incremental git walk for stale/missing-summary cache.** The `stale-summary` and `missing-summary` rules walk only the commits since the last cached HEAD instead of re-walking all of git history on every pre-commit run. Warm runs after a single commit now take milliseconds.
- **Parallel effective-files walk for advisory native rules.** When `--only`/`--exclude` filters are active, the file walk used to build the effective file list runs concurrently with the first group of native rules instead of sequentially after them.
- **Daemon file watcher respects `[walk] exclude`.** Previously the file watcher ignored `[walk] exclude` and registered tens of thousands of inotify watches under directories like `.claude/worktrees/`. The watcher now consults the same gitignore-style matcher as the walker, eliminating ~50k spurious inotify watches on typical projects.
- **Native rule timing diagnostics.** `RUST_LOG=debug normalize rules run --type native` logs per-rule and total elapsed time to stderr via `tracing::debug!`.

### Fixed

- **musl release tarball now ships musl-linked grammars.** The release workflow previously built grammars once on the glibc host runner and bundled the same `.so` files into both the gnu and musl tarballs. The musl-linked `normalize` binary uses the musl loader, which cannot `dlopen()` glibc-linked shared objects, so any grammar load on musl would fail at runtime. The workflow now invokes `cargo xtask build-grammars --target x86_64-unknown-linux-musl --cc musl-gcc` for the musl matrix entry and packs the resulting `target/x86_64-unknown-linux-musl/grammars/*.so` into the musl grammar artifact. A `readelf -d` check in CI fails the build if any grammar still depends on `libc.so.6`. `cargo xtask build-grammars` gained `--target <triple>` and `--cc <compiler>` flags; both default to host behavior so existing invocations are unchanged.
- **libsql `block_on` shim no longer deadlocks/panics when called from a tokio worker.** The cache layers in `normalize-facts`, `normalize-native-rules`, and `normalize-syntax-rules` cache an owned current-thread runtime when constructed from sync code (the common case). Previously the helper used that cached runtime unconditionally, which caused `Cannot start a runtime from within a runtime` panics whenever a `#[tokio::test]` (or any other tokio task) hit a cache that had been initialized earlier from a sync test in the same process. The helper now inspects the call site's tokio context first — only falling back to the cached runtime when not already inside one — so the public API stays synchronous while remaining safe to call from any context.
- **`normalize update` no longer recomputes SHA-256 in O(n²).** The hand-rolled hash implementation in the self-update path was quadratic and could take minutes on macOS releases (~45 MB binary). Replaced with the `sha2` crate; verification is now milliseconds.
- **`DaemonClient` socket path captured at construction, not at every method call.** `DaemonClient::new()` previously re-read `NORMALIZE_DAEMON_CONFIG_DIR` on every method invocation. Under any concurrent use this raced. `DaemonClient::new()` now resolves the env var once and stores the resolved `PathBuf`. New `DaemonClient::with_socket_path(PathBuf)` constructor lets callers (tests, LSP servers talking to multiple workspaces, library embedders) target a specific socket without touching env vars.
- **Daemon refresh events suppressed during first 60 s after `add_root`.** `FileIndex::incremental_refresh()` short-circuited via a `needs_refresh()` staleness gate (a cold-CLI optimization). The daemon's `refresh_root` called the gated variant, killing LSP push diagnostics during the first minute of a daemon session. Adds `FileIndex::incremental_refresh_force()` which skips the gate; the daemon now uses it.
- **Daemon `refresh_root` deadlock on second incremental refresh.** `refresh_root` held the per-root `FileIndex` mutex across the entire match arm, including the save helpers which all re-acquire the same mutex. The lock is now released after the last direct `idx` use and the save helpers re-acquire it as designed.
- **Grammar load failure is a loud warning, not silent empty results.** When `normalize structure rebuild` encounters a file whose grammar `.so` is unavailable, it emits a `tracing::warn!` (once per grammar per run) and skips the file entirely rather than indexing it as having zero symbols. End-to-end fix: `SymbolParser::parse_file` returns `Option<Vec<FlatSymbol>>` (None = grammar unavailable); both the full and incremental paths skip files that return None and never write them to the CA cache or SQLite.
- **JIT compilation re-enabled for Datalog rules.** `ascent-interpreter` upgraded to 0.2.0-alpha.1, which fixes the packed-tuple arity mismatch bug that caused aborts on non-trivial relations. JIT is active by default on x86_64; aarch64 continues to use interpreted evaluation.
- **Daemon memory leak.** `WatchedRoot` no longer holds diagnostics or the reverse-dep graph in memory. After each refresh, issues are persisted to the `daemon_diagnostics` table in the SQLite index, then dropped from heap. The reverse-dep graph is derived transiently from the `imports` table on each refresh and discarded after use. Steady-state daemon RSS reduced from ~2.3 GB (after 10 days) to near-zero.
- **`syntax-rules` walker uses gitignore-style exclude.** Previously the syntax-rules walker matched `[walk] exclude` against filename basenames only, ignoring the gitignore semantics that the rest of the system uses. Now consistent across the codebase.
- **Auto-build index for commands that need it.** Commands that depend on the structural index (`test-gaps`, `blame`, `coupling-clusters`) auto-build the index when it's empty instead of silently returning degraded results. `ensure_ready_or_warn` prints a hint to stderr when the index can't be built.

### Internal

- **`RuleOverride` typed per-rule config.** Rule-specific fields (`filenames`, `paths`, `threshold`) moved out of the flat `RuleOverride` struct into typed per-rule config structs. Common fields (`severity`, `enabled`, `allow`, `tags`) stay shared; rule-specific TOML keys land in `extra` via `#[serde(flatten)]` and are deserialized by each rule via `RuleOverride::rule_config::<T>()`.
- **Refactoring engine** (`refactor/`) — three-layer architecture for composable code transformations: semantic actions (query/mutation primitives), recipes, and a shared executor with dry-run/shadow support. Foundation for `move`, `add-parameter`, `inline-function`, `introduce-variable`, `inline-variable`.
- **`normalize-refactor` crate** — refactoring engine extracted from the main crate. Clean dependency boundary on `normalize-edit`, `normalize-facts`, `normalize-languages`, `normalize-shadow`.
- **`normalize-syntax-rules` `fix` feature gate** — `apply_fixes` and `expand_fix_template` gated behind `default = ["fix"]`. Read-only rules consumers can disable with `default-features = false`.

## [0.3.0] — 2026-05-06

Skipped — see 0.3.1. The 0.3.0 publish was partial (some crates published, others not) due to a CI configuration issue; the `crates.io` graph for 0.3.0 is incomplete and should not be used.

## [0.2.0] — 2026-03-25

### Added

- **`normalize ci`** — single entry point for CI pipelines. Runs syntax, native, and fact rules in one command. Supports `--sarif` output, `--strict` mode (warnings as errors), and `--no-syntax`/`--no-native`/`--no-fact` flags to disable individual engines. Emits a warning diagnostic (rather than failing) when the index has not been built yet.
- **`normalize ratchet`** — metric regression tracking. `ratchet check` compares current metrics (line count, function count, complexity, call-graph complexity) against a stored baseline and fails if any regress; `ratchet update` advances the baseline; `ratchet add`/`remove` manage tracked metrics.
- **`normalize budget`** — diff-based code growth limits. `budget check` enforces per-file or per-directory line/function ceilings relative to a base ref; `budget add`/`update` manage budget entries.
- **Install scripts** with SHA256 checksum verification. `install.sh` (Linux/macOS) and `install.ps1` (Windows) auto-detect platform, fetch the latest release, verify the checksum, install to `~/.local/bin` (Unix) or `%LOCALAPPDATA%\Programs\normalize` (Windows), and hint if the install directory is not on `PATH`. Version pinning via `NORMALIZE_VERSION` env var.
- **JIT compilation** for Datalog rule evaluation on x86_64 Linux and Windows (via `ascent-interpreter` 0.1.5). aarch64 uses interpreted evaluation.
- **Incremental Datalog evaluation**: the daemon now warms the rule engine cache after each index refresh, so subsequent `normalize rules run` calls retract and re-derive only affected strata rather than running a full cold evaluation.

### Improved

- 15 rounds of API polish: cleaner `--help` text, consistent report naming, complete error propagation (no silent swallowing), and improved error messages across all commands.

## [0.1.0] — 2026-02-01

Initial release on [crates.io](https://crates.io/crates/normalize). 38 published crates covering language support (84 languages, ~335 tree-sitter query files), symbol/import/call extraction, Datalog fact rules, syntax linting (94 built-in rules across 13 languages), manifest parsing, output formatting, and the `normalize` CLI binary.
