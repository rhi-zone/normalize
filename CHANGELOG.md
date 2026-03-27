# Changelog

All notable user-facing changes to the Normalize CLI are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`missing-test` fact rule** — flags public functions that are never called from a test function (a function with a test attribute such as `#[test]`, `@test`, `@Test`, or `@pytest.mark`). Disabled by default. Entry-point and module-boundary files excluded via the default allow list.
- **`stale-mock` fact rule** — flags mock/stub functions (identified by attributes such as `@Mock`, `@patch`, `@stub`, `mock`, `stub`, `fake`) that call a callee which no longer exists as a symbol in the index. Catches mocks that were not updated after a rename or deletion. Disabled by default.
- **`normalize grep <path>`** — optional positional `path` argument scopes the search tree (consistent with `view`, `edit`, `rank`). The existing `--root` flag is preserved for backward compatibility; `path` takes precedence when both are given.
- **`normalize rules run --only`/`--exclude`** — glob pattern filtering for which files get diagnostics returned. `--only "*.rs"` restricts to Rust files; `--exclude "tests/"` skips test directories. Applies post-collection across syntax, fact, and native rule engines.
- **`normalize structure rebuild --only`/`--exclude`** — glob pattern filtering for which files get indexed. Files not matching the filter are removed from the index after the walk.
- **`normalize analyze architecture --limit`** — caps the number of `cross_imports` entries in the output (default 20, `--limit 0` disables). Reduces default JSON response from ~196KB to ~10KB, matching the `analyze health --limit` pattern.

### Moved

- **`normalize trend`** — new top-level subcommand for time-series health metrics. Replaces `normalize analyze complexity-trend`, `analyze length-trend`, `analyze density-trend`, `analyze test-ratio-trend`, and `analyze trend`. New names: `normalize trend complexity`, `normalize trend length`, `normalize trend density`, `normalize trend test-ratio`, `normalize trend multi` (all metrics).

### Changed

- **`normalize structure rebuild`** now defaults to incremental mode (mtime-based). Only files changed since the last build are re-indexed. Pass `--full` to force a complete rebuild. When no files have changed, the command prints "Index up to date". The `--json` output includes an `incremental: true` field when incremental mode was used.
- **`normalize view --dir-context`** now accepts an integer `N` instead of a boolean flag. `N` selects context files using Python `list[:N]` semantics on the target→root ordered list: `1` = target dir only, `2` = target + parent, `-1` = all ancestors, `0` = none. Pass the flag without a value to get all ancestors (equivalent to `-1`).
- **`normalize view --dir-context` JSON output** now includes a `dir_context` field in `ViewReport` containing the merged context content. Previously the context was only prepended to text output; agents using `--json` received no context.
- **`normalize rules tags`** now always populates the `rules` array in JSON output (previously the array was empty by default and only filled when `--show-rules` was passed, which made agents misread it as "no rules in this tag"). The `--show-rules` flag has been removed; the rules list is now always included. Text output is unchanged.
- **`normalize syntax ast`** default depth changed from unlimited (`-1`) to `5`. Pass `--depth -1` to restore the old unlimited behavior. This prevents agents from receiving enormous output when inspecting files.
- **`normalize analyze architecture` compact output** no longer truncates hub and symbol paths with opaque worktree-hash prefixes (e.g. `...ba395f/crates/...`). Paths are now shown as clean workspace-relative paths (e.g. `crates/normalize/src/output.rs`).

### Improved

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
