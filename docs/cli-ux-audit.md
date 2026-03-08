# CLI UX Audit

External evaluation of normalize's CLI usability, discoverability, and output quality
relative to standard tools. Generated from a subagent exploration session (2026-03-08).

---

## Bugs to Fix

### 1. `--group-by` flag missing from CLI (high impact)

`normalize sessions stats --group-by project,day` is documented in the rhi ecosystem
CLAUDE.md as the recommended pattern for daily log generation. The backend
(`cmd_sessions_stats_grouped`) is implemented and validated, but the flag is not wired
into the clap CLI struct. Running it produces:

```
error: unexpected argument '--group-by' found
```

Fix: add `--group-by <fields>` to the `sessions stats` subcommand args and call the
grouped variant when it's present.

### 2. `sessions show` / `sessions analyze` ignore `CLAUDE_SESSIONS_DIR`, require correct cwd

`sessions list` and `sessions stats` respect `CLAUDE_SESSIONS_DIR` and have a
`--project` flag. `sessions show <id>` and `sessions analyze <id>` do neither — they
resolve the session path via `git rev-parse --show-toplevel` of the cwd. Running from
a different repo silently looks in the wrong place.

Fix: add `--project <path>` to `sessions show` and `sessions analyze`, matching
`sessions list` behavior. Also ensure both commands respect `CLAUDE_SESSIONS_DIR`.

### 3. `--only <lang>` silently returns zero results

Passing a bare language name to `--only` (e.g. `--only rust`, `--only Rust`,
`--only typescript`) silently returns zero matches. The flag accepts glob patterns
(`*.rs`) or `@alias` names (`@rust`) — but language names are not valid and there's
no error or hint.

Steps to reproduce:
```
normalize grep "fn main" --only rust       # 0 results, no error
normalize grep "fn main" --only "*.rs"     # works
normalize grep "fn main" --only @rust      # works (if alias exists)
```

Fix: either accept bare language names as aliases, or emit an error like:
`'rust' is not a valid pattern — use '*.rs' or '@rust' (see 'normalize aliases')`.

### 4. `normalize analyze complexity <single-file>` silently returns nothing

Passing a single file path returns 0 results with no explanation. The command
requires a directory. This is not documented in `--help`.

Fix: either support single-file input, or emit a clear error when given a file path.

### 5. `view --full` does not show full source

`normalize view --help` says `--full` "Show full source code" but the output is
still a structural outline. Actual source is accessible via the line-range syntax
(`file.rs:1-N`) but `--full` appears to be a no-op or does something undocumented.

Fix: either make `--full` emit the actual source, or remove/rename the flag.

---

## Usability Improvements

### `aliases` subcommand is undiscoverable

The `@rust`, `@typescript`, etc. aliases that make `--only` and `--exclude` work are
only visible if you know to run `normalize aliases`. They are not mentioned in the
`--only` flag help text. Users familiar with ripgrep's `--type` system will expect
`--only rust` to work and get no feedback when it doesn't.

Suggestion: add a note to the `--only`/`--exclude` help text: "Use `*.ext` globs or
`@alias` names (see `normalize aliases`)".

### `analyze hotspots` has no `--limit` flag

It's a ranked list but always returns all results. Other ranked commands (e.g.
`sessions stats`) have `--limit`. Add `--limit N` to `analyze hotspots` and other
ranked `analyze` subcommands.

### Binary not pre-built in fresh checkout

The path `~/git/rhizone/normalize/target/debug/normalize` is referenced in the
ecosystem CLAUDE.md as if it always exists, but a fresh clone has no binary. The
CLAUDE.md in this repo doesn't mention needing `nix develop --command cargo build`
first. An agent picking up normalize tasks will fail if it tries to run the binary
without building first.

Suggestion: document the build step in CLAUDE.md, or add an install/build section
to README. A `just build` or similar would help discoverability.

---

## What Works Well

These are worth preserving and expanding:

- **`normalize view <file>`** — structural outline with line ranges is the single
  most useful feature for LLM-assisted development. A 1000-line file → 20-line
  skeleton an agent can navigate with line-range reads.

- **`normalize sessions stats`** — irreplaceable. Aggregate tool error rates, retry
  hotspots, cache efficiency, and what-if model pricing across hundreds of sessions.
  One concrete finding from the audit run: `ExitPlanMode` fails 96% of the time
  (170/177 attempts); normalize itself generates 23.4M output tokens in retries.
  Nothing in the standard toolchain can surface this.

- **`normalize analyze summary`** — best single-command codebase health overview.
  Health grade + composition breakdown + top concerns in ~20 lines.

- **`normalize sessions messages --role user`** — flat user-prompt extraction with
  timestamps, the right primitive for daily log generation.

- **`normalize grep`** — symbol-annotated matches (`(symbol_name L10-30): match`)
  are better than raw grep for code navigation, even if less composable.

---

## vs. Standard Tools

| Task | normalize | Standard |
|------|-----------|----------|
| Outline large file | `view file.rs` → skeleton + line numbers | `cat` → all lines, or `grep "^pub fn"` → no ranges |
| Session cost/error analysis | `sessions stats` | Impossible without custom JSONL parser |
| Files that change together | `analyze coupling` | Manual `git log` per file |
| Retry hotspots across sessions | `sessions stats --all-projects` | Custom scripting |
| Symbol line range | `view file.rs` | `grep -n` → line number only, no end |
| Codebase LOC breakdown | `analyze size` | `find \| wc -l`, no tree |
| String search | `grep` (slower, non-composable) | `rg`/`grep -rn` (faster, `--type` works) |
| Short file read | `view` (adds overhead) | `cat` (simpler) |
