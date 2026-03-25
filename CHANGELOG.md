# Changelog

All notable user-facing changes to the Normalize CLI are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
