# Changelog

All notable user-facing changes to the Normalize CLI are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`normalize structure search <query>`** — semantic search over the codebase by meaning, not name. Queries a vector embedding index built from symbol chunks (name + signature + doc comment + callers/callees + co-change neighbors). Disabled by default; enable with `embeddings.enabled = true` in `.normalize/config.toml`. Embeddings are generated during `normalize structure rebuild` using fastembed (ONNX, no server or API key required). Default model: `nomic-embed-text-v1.5` (768 dims). Results are ranked by cosine similarity with a staleness penalty applied at query time. `--json` and `--jq` work like every other command. `SearchReport.ann_used` indicates whether the ANN index was used.
- **ANN search via sqlite-vec** — semantic search now uses a `vec_embeddings` virtual table (`vec0`) for approximate nearest-neighbor lookup instead of loading all vectors into memory. The extension is registered per-connection via a raw FFI handle (`VecConnection`); the top-50 ANN candidates are then re-ranked by the staleness penalty. Falls back to brute-force if the extension is unavailable (e.g. first run before schema migration).
- **`normalize init` suggests semantic search** — `normalize init` (not `--setup`) now prints a one-line CTA to enable semantic search after initializing the project.

- **No `git` binary required** — all git operations now use `gix` (pure-Rust gitoxide). This includes `git blame` (ownership, provenance, `view history`), `git status --porcelain` (stale-summary uncommitted change detection, `git.dirty`/`git.staged` rule source), path-filtered commit counts and last-commit lookup (`stale-summary`/`missing-summary` incremental cache), `git rev-list --count` (coupling-clusters threshold), budget metrics diff (`normalize budget check/measure`), and ratchet ref-based check/measure (`normalize ratchet check --baseline-ref`, `normalize ratchet measure --diff-ref`). A `git` binary in `$PATH` is no longer a runtime dependency for any normalize operation.

- **Daemon-cached diagnostics for all engines** — the daemon now caches syntax, fact, and native rule diagnostics and serves them instantly on `normalize rules run`. Cache is primed eagerly on file changes (incremental for syntax/fact, full re-run for native) and lazily on first request. When the daemon is running, `rules run` gets pre-computed results instead of running expensive local evaluation. Falls back to local evaluation transparently when the daemon is unavailable.
- **`missing-test` fact rule** — flags public functions that are never called from a test function (a function with a test attribute such as `#[test]`, `@test`, `@Test`, or `@pytest.mark`). Disabled by default. Entry-point and module-boundary files excluded via the default allow list.
- **`stale-mock` fact rule** — flags mock/stub functions (identified by attributes such as `@Mock`, `@patch`, `@stub`, `mock`, `stub`, `fake`) that call a callee which no longer exists as a symbol in the index. Catches mocks that were not updated after a rename or deletion. Disabled by default.
- **`normalize config validate` deep validation** — now runs four validation phases (TOML syntax, JSON Schema compliance, serde deserialization as `NormalizeConfig`, rules config parsing) on both project and global config files. Reports errors with file path, line/column when available, and validation phase. Exits non-zero on errors for CI/hook use.
- **`normalize grep <path>`** — optional positional `path` argument scopes the search tree (consistent with `view`, `edit`, `rank`). The existing `--root` flag is preserved for backward compatibility; `path` takes precedence when both are given.
- **`normalize rules run --files`** — accept an explicit list of file paths, bypassing the file tree walker entirely. Critical for hook-grade latency where the caller already knows which files changed. Composes with `--only`/`--exclude` for further filtering. Threaded through syntax rules, and native threshold rules (long-file, high-complexity, long-function).
- **`normalize rules run --only`/`--exclude`** — glob pattern filtering for which files get diagnostics returned. `--only "*.rs"` restricts to Rust files; `--exclude "tests/"` skips test directories. Applies post-collection across syntax, fact, and native rule engines.
- **`normalize structure rebuild --only`/`--exclude`** — glob pattern filtering for which files get indexed. Files not matching the filter are removed from the index after the walk.
- **`normalize analyze architecture --limit`** — caps the number of `cross_imports` entries in the output (default 20, `--limit 0` disables). Reduces default JSON response from ~196KB to ~10KB, matching the `analyze health --limit` pattern.
- **Co-change edge index** — `normalize structure rebuild` now populates a `co_change_edges` table in the SQLite index with file pairs that frequently change together (co-change count ≥ 2, commits touching >50 files skipped as noise, per-file fanout capped at top 20 partners). Incremental: only new commits since the last rebuild are processed. `normalize analyze coupling-clusters` queries this table instead of re-walking git history on every invocation; falls back transparently to the git walk with a warning when the table is empty. Rebuild output now includes a `co_change_edges` count.
- **`stale-doc` native rule** — detects documentation files that are likely stale because strongly co-changed code files have been updated more recently. Queries the `co_change_edges` index for each doc file (`**/*.md`, `**/*.rst`, `docs/**/*`), finds code files it historically changes with, and flags the doc if any partner was committed more recently. `SUMMARY.md` is excluded (covered by `stale-summary`). Configurable via `[rules."stale-doc"]` with `min_co_changes` (default 3), `min_lag_days` (default 0), and `doc_patterns`. Disabled by default; requires `normalize structure rebuild` to populate the index.

### Fixed

- **Auto-build index for commands that need it** — commands that depend on the structural index (`test-gaps`, `blame`, `coupling-clusters`, symbol search) now auto-build the index when it's empty instead of silently returning degraded results. `ensure_ready_or_warn` prints a hint to stderr when the index can't be built (e.g. indexing disabled) so results are never silently empty. `test_gaps` uses `ensure_ready` (auto-build with error on failure) since call graph data is essential for meaningful results.
- **sqlite-vec / libsql initialization crash** — `register_vec_extension()` used `sqlite3_auto_extension` which internally calls `sqlite3_initialize()`, conflicting with libsql's own initialization (assertion failure: threading config mismatch). Replaced with per-connection registration via `VecConnection`: a raw FFI handle that calls `sqlite3_vec_init` directly on each connection, avoiding the global auto-extension mechanism entirely.
- **Semantic embedding rebuild performance** — full rebuild now drops and recreates embedding tables instead of doing per-row deletes, avoiding massive SQLite bloat (675MB on disk for 75MB of data). Incremental updates use `INSERT OR REPLACE` with a UNIQUE constraint instead of SELECT-DELETE-INSERT per row. `VACUUM` runs after a full rebuild to reclaim dead pages.
- **Semantic embedding progress output** — `structure rebuild` with embeddings enabled now prints progress to stderr: model loading, symbol count, per-batch progress ("Embedded 64/9051 symbols"), and a final summary with elapsed time.

### Moved

- **`normalize analyze length`** → **`normalize rank length`** — ranking command (longest functions by line count) now lives under `rank` where ranked-list commands belong.
- **`normalize analyze test-gaps`** → **`normalize rank test-gaps`** — ranking command (public functions lacking test coverage, ranked by risk score) now lives under `rank`.
- **`normalize analyze node-types`** removed — duplicate of `normalize syntax node-types` which already existed. Use `normalize syntax node-types` instead.
- **`normalize trend`** — new top-level subcommand for time-series health metrics. Replaces `normalize analyze complexity-trend`, `analyze length-trend`, `analyze density-trend`, `analyze test-ratio-trend`, and `analyze trend`. New names: `normalize trend complexity`, `normalize trend length`, `normalize trend density`, `normalize trend test-ratio`, `normalize trend multi` (all metrics).

### Changed

- **`normalize rules run --only`/`--exclude` pre-walk scoping** — glob patterns are now applied *before* file parsing and walking, not just after. Syntax rules skip non-matching files in `collect_source_files`; advisory native rules (long-file, high-complexity, long-function) receive a pre-filtered file list. The post-walk filter is kept as a safety net. Single-file `--only` runs are now proportional to the matched file count, not the full tree.

### Internal

- **`long-file`, `high-complexity`, `long-function` native rules** — threshold-based health findings from `analyze health` are now available as native rules in `normalize rules run --type native`. Default disabled (advisory); enable via `[rules."long-file"] enabled = true` in config or `--rule long-file` on the command line. Defaults: 500 lines (long-file), cyclomatic complexity 20 (high-complexity), 100 lines (long-function). `NativeRuleDescriptor` gained `default_enabled` field; `--rule <id>` implicitly enables the targeted rule. Thresholds configurable per-rule via `threshold` key in config (e.g. `[rules."long-file"] threshold = 1000`).
- **`large-file` rule renamed to `long-file`** — consistency with `long-function` (both measure length). Config key `[rules."large-file"]` must be updated to `[rules."long-file"]`.
- **`RuleOverride` typed per-rule config** — rule-specific fields (`filenames`, `paths`, `threshold`) moved out of the flat `RuleOverride` struct into typed per-rule config structs. Common fields (`severity`, `enabled`, `allow`, `tags`) stay shared; rule-specific TOML keys land in `extra` via `#[serde(flatten)]` and are deserialized by each rule via `RuleOverride::rule_config::<T>()`.
- **Refactoring engine** (`refactor/`) — new three-layer architecture for composable code transformations: semantic actions (query/mutation primitives), recipes (rename is the first), and a shared executor with dry-run/shadow support. `edit rename` is now a thin wrapper over `refactor::rename::plan_rename` + `RefactoringExecutor::apply`, reducing `do_rename` from ~270 lines to ~75. Foundation for future `move`, `extract`, and `inline` commands.
- **`normalize-refactor` crate** — refactoring engine extracted from the main crate into `crates/normalize-refactor/`. Clean dependency boundary: depends only on normalize-edit, normalize-facts, normalize-languages, normalize-shadow. `plan_rename` now accepts pre-resolved path components instead of a raw target string, decoupling from path resolution.
- **`normalize-syntax-rules` `fix` feature gate** — `apply_fixes` and `expand_fix_template` gated behind `default = ["fix"]`. Read-only rules consumers can disable with `default-features = false`. Establishes the boundary for future `normalize-refactor` integration.

### Changed

- **Incremental syntax rules** — `normalize rules run --type syntax` now caches per-file results in `.normalize/syntax-cache.json` keyed by file mtime (nanosecond precision). Unchanged files are skipped on subsequent runs, cutting repeat run time dramatically on large codebases.

- **`normalize rules run` routes through daemon when running** — if `normalize daemon start` is active, `normalize rules run` (and any invocation that hits fact rules) sends the request to the daemon via Unix socket and receives pre-warmed Datalog evaluation results instead of cold-evaluating from scratch (~45 seconds on large codebases). Falls back to cold evaluation transparently when no daemon is running. A new `RunRules` request type is added to the daemon protocol.

- **`normalize analyze docs --json`** `by_language` field now serializes as named objects `{"documented": N, "total": N}` instead of positional arrays `[N, N]`.
- **`normalize grammars list --json`** now returns objects with `name` and `path` fields instead of bare strings. Text output is unchanged.

- **`normalize structure rebuild`** now defaults to incremental mode (mtime-based). Only files changed since the last build are re-indexed. Pass `--full` to force a complete rebuild. When no files have changed, the command prints "Index up to date". The `--json` output includes an `incremental: true` field when incremental mode was used.
- **`normalize view --dir-context`** now accepts an integer `N` instead of a boolean flag. `N` selects context files using Python `list[:N]` semantics on the target→root ordered list: `1` = target dir only, `2` = target + parent, `-1` = all ancestors, `0` = none. Pass the flag without a value to get all ancestors (equivalent to `-1`).
- **`normalize view --dir-context` JSON output** now includes a `dir_context` field in `ViewReport` containing the merged context content. Previously the context was only prepended to text output; agents using `--json` received no context.
- **`normalize rules tags`** now always populates the `rules` array in JSON output (previously the array was empty by default and only filled when `--show-rules` was passed, which made agents misread it as "no rules in this tag"). The `--show-rules` flag has been removed; the rules list is now always included. Text output is unchanged.
- **`normalize syntax ast`** default depth changed from unlimited (`-1`) to `5`. Pass `--depth -1` to restore the old unlimited behavior. This prevents agents from receiving enormous output when inspecting files.
- **`normalize analyze architecture` compact output** no longer truncates hub and symbol paths with opaque worktree-hash prefixes (e.g. `...ba395f/crates/...`). Paths are now shown as clean workspace-relative paths (e.g. `crates/normalize/src/output.rs`).

### Improved

- **`normalize context` compact output** now includes `<!-- source -->` file path comments and `---` separators between blocks when multiple context files are merged. Single-block output is unchanged.
- **`normalize ci` / `normalize rules run` compact output** `(N files)` header now reads `(N files checked)` to clarify it is the number of files scanned, not files with issues.
- **`normalize package tree --depth N`** — new flag caps the dependency tree at depth `N` (0 = roots only). Limits both text and JSON output. Default: unlimited (current behavior).
- **`normalize grep`** — consecutive matches within the same symbol are now grouped under a single `(SymbolName L48-61):` header rather than repeating the symbol tag on every line.
- **`normalize view <file>:N-M`** — header no longer duplicates the line range (was `file.rs:10-20:10-20`, now `file.rs:10-20`).

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
