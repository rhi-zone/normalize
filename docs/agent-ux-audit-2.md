# Agent UX Audit — Pass 3 (Follow-up)

Verification audit covering commands changed since Pass 2 (2026-03-21). Goal: confirm fixes land as expected, identify regressions or incomplete fixes, and surface any new issues.

Date: 2026-03-27

## What Changed Since Pass 2

From CHANGELOG and task description:

- `rules show` / `rules tags` — structured JSON (was text blob)
- `daemon list` — exits 0 when not running (was exit 1)
- `syntax ast --depth` — depth limit added
- `syntax query --compact` — improved (was count-only)
- `analyze health --limit` — large-file array truncation
- `package list` — `ecosystems_detected` in JSON
- `analyze architecture --compact` — improved format
- `view --dir-context` — context file prepending
- `rules run` / `structure rebuild` / `grep` — `--only` / `--exclude` filtering

---

## Per-Command Findings

### `rules show` — FIXED

**Before:** `{"message": "<full human text>", "success": true}` — unstructured text wrapper.

**After:** Fully structured `RuleInfoReport`: `{id, rule_type, severity, enabled, builtin, tags, languages, message, allow, fix, description}`. The `description` field contains the full markdown documentation; `fix` is the auto-fix string or null; `allow` is the glob allow-list.

**Rating: GOOD.** The schema is well-documented via `--output-schema`. All machine-relevant fields are top-level scalars or typed arrays.

**Minor note:** `rules show nonexistent-rule` returns an error string on stderr and exits 1, with no JSON body. Agents using `--json` get no structured error. Low-impact since the use case is rare.

---

### `rules tags` — FIXED

**Before:** `{"message": "<compact text>", "success": true}` — unstructured text wrapper.

**After:** `{"tags": [{tag, source, count, rules: []}]}` — structured. The `rules` array is empty by default and populated only when `--show-rules` is passed.

**Rating: GOOD.**

**Caveat:** An agent receiving `rules: []` for every tag in the default JSON response may not realize the array can be populated by passing `--show-rules`. The empty array looks like "no rules in this tag" rather than "rules not requested." Consider always populating `rules` in JSON mode, since the cost is marginal and the confusion is real. This is a low-priority UX polish issue.

---

### `daemon list` — FIXED

**Before:** Exit code 1 with stderr error when daemon not running.

**After:** `{"roots": [], "running": false}` with exit code 0 when daemon not running. Confirmed working.

**Rating: GOOD.**

---

### `syntax ast --depth` — FIXED

**Before:** No `--depth` flag; 500KB+ JSON for a 200-line file; guide docs referenced `--depth` flag that didn't exist.

**After:** `-d/--depth <N>` flag implemented. With `--depth 2`, a 200-line file goes from 100KB JSON / 44KB compact to 3KB / 1.3KB respectively. Default is `-1` (unlimited).

**Compact mode improved:** `--compact` now emits `(node_kind) [L1-L5]` type-outline format (no source text), making it usable for structure overview. With `--depth 2 --compact` an agent gets a 1.3KB file-structure summary.

**Rating: GOOD.**

**Remaining issue (Low):** Without `--depth`, output is still large (100KB JSON for a small file). The default of `-1` (unlimited) means agents who forget the flag still get a wall of output. A default depth of 5–10 might be a better agent default; current unlimited default requires agents to always remember to pass `--depth`. Not a blocker since the flag exists.

---

### `syntax query --compact` — FIXED

**Before:** Returned only a count string (`"4 matches"`) — no file, line, or capture data.

**After:** Emits `file:line: @capture = text` per match. Example:
```
crates/normalize/src/main.rs:3: @name = handle_schema_flag
crates/normalize/src/main.rs:24: @name = reset_sigpipe
```

JSON output is unchanged and still well-structured: `[{file, grammar, kind, start_row, start_col, end_row, end_col, text, captures: {name: text}}]`.

**Rating: GOOD.**

**Note:** The compact format uses `@capture_name = value` which is clear and parseable. For queries with multiple captures, each capture appears as a separate output line for the same match location — agents processing compact output should be aware of this.

---

### `analyze health --limit` — FIXED

**Before:** JSON response was 150–180KB due to unbounded `large_files` array.

**After:** Default `--limit 10` caps `large_files` at 10 entries. JSON response is now ~3KB (default) vs ~153KB (`--limit 0`). The flag is documented in `--help`.

**Rating: GOOD.**

**Note:** The `--limit` flag name is also documented as "Maximum number of large files to include in output (0 = no limit, default 10)". Clear and discoverable.

---

### `package list --json` — FIXED

**Before:** Multi-ecosystem advisory appeared only in compact text output; JSON consumers received partial results with no indication other ecosystems were skipped.

**After:** `{"ecosystem": "cargo", "ecosystems_detected": ["cargo", "npm", "nix"], "packages": []}` — `ecosystems_detected` field is now in the JSON response.

**Rating: GOOD.**

**Note:** The `note:` / `hint:` advisory lines still print to stderr (not stdout), which is correct. Confirmed that `--json` stdout is clean JSON; the advisory is properly on stderr.

---

### `analyze architecture --compact` — IMPROVED

**Before:** Compact output printed only a `## Cross-Imports` section — a list of bidirectional coupling pairs. Hub modules, layer flows, symbol hotspots, and orphan modules were absent.

**After:** Compact output now uses labeled prefix format:
- `HUBS: path (N dependents), ...` — top hub files
- `LAYERS: a → b (N imports)` — one line per layer flow
- `COUPLING: path ↔ path (N/N imports)` — one line per bidirectional pair (979 lines for this repo)
- `SYMBOLS: path:symbol (N callers), ...` — top symbol hotspots
- `ORPHANS: path, path, ...` — isolated modules
- `SUMMARY: N modules, N symbols, N imports (N resolved), N cross-imports, N orphans`

All sections are machine-parseable with their keyword prefixes.

**Rating: MIXED (improved from POOR).** The format is now genuinely useful for agents — HUBS, LAYERS, and SUMMARY are the most actionable lines and they're present. The COUPLING section is 979 lines for this repo — agents reading the full compact output may still want to filter to specific sections.

**Remaining issues:**

1. **Path truncation in HUBS and SYMBOLS lines.** Paths are truncated to a short suffix (e.g., `...ba395f/crates/normalize/src/output.rs`). The `ba395f` fragment is a worktree hash — agents cannot resolve truncated paths to actual filesystem locations. Full paths are available in the JSON (`coupling_hotspots[].path`) and in COUPLING lines (which are full paths).

2. ~~**JSON remains large (196KB).**~~ **FIXED** — `-l/--limit N` flag added (default 20). `cross_imports` is truncated to 20 entries by default; `--limit 0` disables the cap. JSON output is now ~few KB at default limit. Worktree hash fragments in paths are also cleaned from compact output.

---

### `view --dir-context` — PARTIAL

**Claim:** `--dir-context` accepts an integer `N` and prepends context files.

**Actual behavior:** The flag prepends `.context.md` / `CONTEXT.md` files from ancestor directories (up to N levels). This repo has no `.context.md` files, so the flag is a silent no-op in the normalize codebase.

**What an agent should know:**
- `--dir-context` only works when `.context.md` or `CONTEXT.md` files exist in the directory hierarchy
- The flag has **no effect on `--json` output** — context is prepended as text to the text output only via `view_prefix`, which is not part of `ViewReport`
- An agent using `--json --dir-context` gets the same JSON as without it; the context is not machine-accessible

**Rating: MIXED for agent use.** The feature works as designed for text output in repos that use `.context.md` conventions. For agents using `--json`, `--dir-context` is a no-op. Agents that want directory context for a symbol lookup should use `view <dir>` separately and combine the results.

**Issue (Medium):** `--dir-context` context is not exposed in JSON. An agent that wants the SUMMARY.md or .context.md content for a directory alongside a view result has no single-call way to get it. The `ViewReport.summary` field exists in the schema but is only populated for directory and file views (not for symbol views), and is not populated by `--dir-context`. Consider populating `ViewReport.context_files: [{path, content}]` from `--dir-context` in JSON mode.

---

### `rules run --only` / `--exclude` — FIXED

Both flags work as described. `--only "*.rs"` filters results to only Rust-related diagnostics; `--exclude "tests/"` skips test directories.

**`files_checked` now reflects filtered files (FIXED).** Previously `files_checked` counted all files with violations before the filter was applied. Now `files_checked` is recomputed after `--only`/`--exclude` filtering to count only unique files in the remaining issues.

**JSON structure:** `{"files_checked": N, "issues": [{file, line, column, end_line, end_column, rule_id, severity, message, source}]}`. Clean and complete.

**Rating: GOOD.**

---

### `structure rebuild --only` / `--exclude` — NEW

Not directly tested (runs in background against SQLite). Per CHANGELOG: "Files not matching the filter are removed from the index after the walk." This means `--only "*.rs"` creates a partial index with only Rust files — subsequent cross-file queries will have gaps. An agent running `structure rebuild --only "*.rs"` then `analyze architecture` will get architecture results only for Rust files.

**Rating: Functional** for scoped indexing. Agents should document the partial-index implication when using this flag.

---

### `grep --only` / `--exclude` — NEW, GOOD

Confirmed working. `grep "OutputFormatter" --only "*.rs"` returns only Rust-file matches. The flag is documented in `--help` examples.

**Rating: GOOD.**

---

## Summary: What Was Fixed vs. Remaining Issues

### FIXED since Pass 2

| Issue | Command | Status |
|-------|---------|--------|
| `rules show --json` returned text blob `{message: string}` | `rules show` | **FIXED** — structured `RuleInfoReport` |
| `rules tags --json` returned text blob | `rules tags` | **FIXED** — structured `{tags: [{tag, source, count, rules}]}` |
| `daemon list` exits 1 when daemon not running | `daemon list` | **FIXED** — exits 0 with `{running: false, roots: []}` |
| No `--depth` flag on `syntax ast`; guide docs wrong | `syntax ast` | **FIXED** — `--depth` added; compact mode useful |
| `syntax query --compact` returned count only | `syntax query` | **FIXED** — `file:line: @capture = text` per match |
| `analyze health --json` unbounded at 180KB | `analyze health` | **FIXED** — default `--limit 10` reduces to ~3KB |
| `package list --json` drops multi-ecosystem advisory | `package list` | **FIXED** — `ecosystems_detected` field in JSON |
| `analyze architecture --compact` omits hub modules, layer flows | `analyze architecture` | **IMPROVED** — now has HUBS, LAYERS, SYMBOLS, ORPHANS, SUMMARY |
| `analyze architecture --json` is 196KB; `cross_imports` ~190KB; no `--limit N` | `analyze architecture` | **FIXED** — `-l/--limit N` added (default 20); `--limit 0` disables cap |
| `rules run --only` filters output but `files_checked` still shows all files checked | `rules run` | **FIXED** — `files_checked` recomputed from filtered issues after `--only`/`--exclude` |

### REMAINING ISSUES

| # | Issue | Command | Severity |
|---|-------|---------|----------|
| 2 | HUBS and SYMBOLS paths in compact output are truncated (`...hash/path`) — not resolvable | `analyze architecture` | **Medium** — truncated paths lose actionability |
| 3 | `--dir-context` context is text-only; not exposed in `ViewReport` JSON | `view` | **Medium** — agents using `--json` get no context |
| 4 | `rules tags --json` default has `rules: []` for all tags; `--show-rules` flag not obvious | `rules tags` | **Low** — agents may read empty array as "no rules" |
| 5 | `syntax ast` default depth is unlimited; agents must remember `--depth` flag for large files | `syntax ast` | **Low** — flag exists but default is footgun |

### PREVIOUSLY KNOWN ISSUES NOT RE-EVALUATED

(These were in Pass 2 and have not been re-checked in this pass — still presumed open)

- `package tree --json` no `--depth` flag (full transitive closure, 122KB+)
- `analyze docs` JSON uses positional `[N, N]` array for `by_language` instead of named fields
- `context` compact no per-file delimiter in multi-file repos
- `ci` compact summary "N files" means files-checked, not files-with-issues
- `grammars list --json` has no `path` per grammar
- `aliases` compact `+N` truncation

---

## Overall Assessment

Seven of the eight Pass 2 high/medium priority fixes are confirmed working. The most impactful fix was `analyze health --limit` — the default-10 cap brings JSON from 180KB to 3KB, making the command practical for agents. The `syntax ast --depth` and `syntax query --compact` fixes similarly cross the practical-use threshold.

The `analyze architecture --json` size problem (196KB) is the highest-priority remaining issue. The compact output is now much better — keyword-prefixed lines that agents can parse — but the JSON remains impractical. A `--top N` or `--limit N` flag on each section (defaulting to something like 20) would fix this.

The `view --dir-context` fix is architecturally incomplete: the feature works for text output but has no JSON affordance. This limits its usefulness for agents that use `--json` for most commands.

### Templates to Follow

The best-designed commands in the surface remain `ratchet measure`, `budget measure`, and `config validate` — single-line compact with labeled scalars, clean JSON, exit code carries the error signal. The `syntax ast --depth 2 --compact` pattern has now joined this list as the recommended way for agents to get file structure without token overhead.
