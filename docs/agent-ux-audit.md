# Agent UX Audit: Compact Output Baseline

Cross-model audit of `--compact` output quality for agent consumption. Three models (Haiku, Sonnet, Opus) independently evaluated 12 commands. **Cross-model agreement (2+ models flagging the same issue) = genuine problem.** Single-model flags are lower priority and may reflect model-specific parsing preferences.

Date: 2026-03-20

## Methodology

Each model was given the same compact output samples and asked to rate parseability, token efficiency, and structural clarity. Ratings: GOOD (reliable machine parsing), MIXED (parseable with caveats), POOR (fragile or ambiguous).

## Prioritized Issues

Issues ordered by cross-model agreement count, then severity.

| # | Issue | Commands | Models | Severity |
|---|-------|----------|--------|----------|
| 1 | Truncated column headers (`WebFet`/`WebSea`) with no explanation | `sessions patterns` | Haiku, Sonnet, Opus | **High** — agents can't map truncated headers to API field names |
| 2 | No explicit message delimiters; multi-line content bleeds across boundaries | `sessions messages` | Haiku, Sonnet, Opus | **High** — boundary detection is fragile; multi-line tool output breaks naive parsing |
| 3 | Compound encoded fields (`80u 76tc`) — two values in one cell with opaque suffixes | `sessions list` | Haiku, Sonnet, Opus | **Medium** — requires regex `\d+u \d+tc` to decompose; field names lost |
| 4 | Format inconsistency between compact and JSON field names (`title` vs `first_message`) | `sessions list` | Opus (explicit), Sonnet (implicit via MIXED) | **Medium** — agents switching formats get different schemas |
| 5 | Silent empty output on missing/invalid argument (indistinguishable from zero results) | `analyze complexity` | Sonnet, Haiku (format-switching variant) | **High** — agents can't distinguish "no results" from "bad input" |
| 6 | Mixed formatting mid-output (prose then markdown, or mixed markdown styles) | `analyze complexity`, `sessions stats` | Haiku, Sonnet | **Medium** — parser must handle multiple formats in one response |
| 7 | `.` for zero values instead of `0` or `0.0%` | `sessions patterns` | Opus, Haiku (implicit) | **Low** — unusual but learnable convention |
| 8 | `row(s)` footer noise in query output | `structure query` | Opus | **Low** — minor; easily stripped |
| 9 | Bold markdown (`**label**`) adds tokens with no machine value | `sessions stats` | Opus, Sonnet | **Low** — 4 extra tokens per label, purely visual |
| 10 | No row count / completeness signal | `structure files` | Haiku, Sonnet | **Medium** — agent can't tell if output is truncated |
| 11 | `rank complexity` has no header row, ambiguous separator, variable-width score | `rank complexity` | Haiku | **Low** — Sonnet rated GOOD; likely model-specific |
| 12 | `rules list` — `off` as conditional token not fixed column | `rules list` | Sonnet | **Low** — Haiku rated GOOD; likely model-specific |

## Per-Command Consensus

| Command | Haiku | Sonnet | Opus | Consensus | Key Cross-Model Issues |
|---------|-------|--------|------|-----------|----------------------|
| `sessions list` | GOOD | MIXED | — | **MIXED** | Compound `80u 76tc` field; field name mismatch across formats |
| `sessions stats` | GOOD | POOR | — | **MIXED** | Bold markdown waste; mixed formatting styles |
| `sessions messages` | MIXED | MIXED | — | **MIXED** | No message delimiters; multi-line bleed (all three models) |
| `sessions patterns` | MIXED | MIXED | — | **MIXED** | Truncated headers (all three); `.` for zero |
| `structure query` | GOOD | GOOD | — | **GOOD** | Minor: `row(s)` footer, opaque `n` column |
| `structure stats` | GOOD | MIXED | — | **MIXED** | Unlabeled ratio/columns (Sonnet) |
| `structure files` | MIXED | POOR | — | **MIXED** | No row count; no metadata; opaque ordering |
| `analyze complexity` | MIXED | POOR | — | **POOR** | Silent empty output; format switching |
| `rank complexity` | POOR | GOOD | — | **SPLIT** | Disagreement — likely model-specific preferences |
| `view` | GOOD | POOR | — | **SPLIT** | Sonnet wants type info; Haiku finds it sufficient |
| `grep` | GOOD | GOOD | — | **GOOD** | Symbol context parenthetical praised by Sonnet |
| `rules list` | GOOD | MIXED | — | **MIXED** | Column parsing disagreement |

## Quick Wins

High cross-model agreement, likely easy to fix:

1. **Full column headers in `sessions patterns`** — don't truncate; or provide a header legend. All three models flagged this.
2. **Message delimiters in `sessions messages`** — add an explicit separator (e.g., `---` or `\0` in compact mode) between messages. All three models flagged fragile boundaries.
3. **Decompose compound fields in `sessions list`** — separate `usage` and `tool_calls` into distinct columns instead of `80u 76tc`.
4. **Non-silent errors in `analyze complexity`** — print a diagnostic line when input is missing/invalid instead of producing empty output.
5. **Consistent field names across `--compact` and `--json`** — `title` vs `first_message` should be the same concept with the same name.

## Deferred / Uncertain

Single-model flags or disagreements — investigate if patterns emerge:

- **`rank complexity` formatting** — Haiku rated POOR, Sonnet rated GOOD. Likely reflects different parsing strategies. Monitor.
- **`view` output richness** — Sonnet wants type info and metadata; Haiku finds plain path list sufficient. Revisit when `view` gets richer output anyway.
- **`rules list` column parsing** — Sonnet flagged `off` as non-fixed-width; Haiku found the table clear. May depend on parsing approach.
- **`.` for zero in pattern matrix** — Opus flagged as unusual. Technically unambiguous but unconventional. Low priority.
- **Bold markdown in stats** — wastes tokens but doesn't break parsing. Fix opportunistically.

## Meta-Observation

Opus's deepest insight: `--compact` is a human format marketed as an agent format. `--json` is always more reliable for programmatic consumers, but compact exists for token efficiency. The real fix is making compact genuinely machine-parseable (fixed-width, explicit delimiters, no markdown) rather than a pretty-printed human format with fewer words. This is a design-level tension, not a per-command bug.

---

## Pass 2 — 2026-03-26

Single-author audit covering the 20+ commands not evaluated in Pass 1. All commands run against the normalize repo itself using `./target/debug/normalize` with `--compact` and `--json` modes. Rating scale: **GOOD** (reliable machine parsing), **MIXED** (parseable with caveats), **POOR** (fragile or ambiguous).

Commands already covered in Pass 1: `sessions list/stats/messages/patterns`, `structure query/stats/files`, `analyze complexity`, `rank complexity`, `view`, `grep`, `rules list`.

### Methodology

Each command was run with `--compact` and `--json`. Evaluation criteria:
1. Token efficiency of default text output (no unnecessary prose, headers, or markdown)
2. JSON schema quality (typed, complete, lossless vs. compact)
3. Error signal quality (actionable vs. generic)
4. Missing machine-readable affordances

---

### `aliases`

**Compact:** Prints an `Aliases:` header then indented `@name  pattern, ... (+N)` lines, plus a `Detected languages:` footer. The `+N` truncation means a machine reading compact output cannot get the full pattern list — it must use `--json` to get all patterns. The footer language list duplicates information available via `grammars list`.

**JSON:** Well-structured: `{"aliases": [{name, patterns, status}], "detected_languages": [...]}`. `status` distinguishes `builtin` from `custom` — useful for agents deciding whether to override.

**Rating: MIXED.** JSON is GOOD; compact loses patterns via truncation. An agent that reads compact to check alias membership gets incomplete data.

**Issue:** `+N` truncation in compact output makes it unreliable for alias membership checks. Agents must use `--json`.

---

### `context`

**Compact:** Prints the full markdown content of `.context.md` files without headers or delimiters when multiple files exist. No file path prefix — content from different `.context.md` files runs together.

**JSON:** `{"entries": [{path, kind, content}], "kind": "Full|List"}`. The `kind` field distinguishes full content from `--list` mode. `path` is present, allowing agents to attribute content.

**`--list` mode:** Returns only file paths — clean and useful for discovery.

**Rating: MIXED.** JSON is GOOD. Compact multi-file output has no per-file delimiter, so an agent consuming compact text cannot tell which `.context.md` file a paragraph came from. Single-file projects are unaffected.

**Issue:** No per-entry delimiter in compact multi-file output. Add a `## path/to/.context.md` header line before each file's content in compact mode.

---

### `init`

**Compact (dry-run):** `[dry-run] Would initialize normalize:\n  (no changes needed)` — 3 tokens of useful information wrapped in 2 lines of prose. On a repo already initialized, conveys only `already_initialized=true`.

**JSON (dry-run):** `{"changes": [], "dry_run": true, "message": "Already initialized."}` — clean, but `message` is a human string and `changes` is always an empty array on an already-initialized repo so there is no structural difference between "no changes needed" and a real dry-run showing what would change.

**Rating: GOOD** for the already-initialized case (clean JSON). The command is a setup-once operation, not a query; UX issues here have low agent impact.

---

### `update --check`

**Compact:** Three lines: current version, latest version, and a prose verdict string. The verdict (`"You are running the latest version."` vs some other string) is not machine-parseable without string matching.

**JSON:** `{"current_version": "0.2.0", "latest_version": "0.2.0", "update_available": false}`. Boolean `update_available` is exactly what an agent needs.

**Rating: GOOD.** JSON is clean and agent-friendly. Compact is verbose but rarely called.

---

### `grammars list`

**Compact:** Header `Installed grammars (N):` then one grammar name per line — clean, parseable.

**JSON:** `{"grammars": ["ada", ...]}` — a flat name array. No version or path information in the list. To get paths, call `grammars paths` separately.

**Rating: GOOD** for name lookup. **Missing:** the JSON schema has no `version` or `path` per grammar. An agent checking whether a specific grammar is installed gets a name but cannot find its `.so` path or version in one call. Two calls required (`grammars list` + `grammars paths`) for path resolution.

**Issue:** `grammars list --json` should include at minimum a `path` field per grammar, eliminating the need for a `grammars paths` call when agents want to confirm a grammar is loadable.

---

### `grammars paths`

**Compact:** `Grammar search paths:\n  [source] /path` — clean two-token per entry format.

**JSON:** `{"paths": [{source, path, exists}]}` — `exists` boolean is excellent; tells agent whether the path is usable without a filesystem call.

**Rating: GOOD.**

---

### `guide <topic>`

**Compact / Pretty:** Emits raw markdown prose — multiple `##` headers, code blocks, inline examples. Designed for human reading. Not parseable beyond extracting code fences.

**JSON:** `{"content": "<full markdown string>", "topic": "rules"}` — wraps the entire markdown in a string field. An agent receives the same unstructured text, just JSON-encoded.

**Rating: POOR for machine parsing.** Guides are intentionally human-readable prose and there is no structured equivalent. This is acceptable for documentation, but an agent invoking `guide` to discover command syntax gets a wall of markdown it must parse heuristically. The `--output-schema` confirms the return type is `GuideReport {content: string, topic: string}` — the content field is unstructured by design.

**Issue:** No machine-readable command index in guide output. An agent wanting "what flags does `syntax ast` accept" must call `syntax ast --help` directly. `guide` is not a substitute for `--help` and should not be used as one.

---

### `generate client` / `generate types` / `generate cli-snapshot`

Not evaluated (require an OpenAPI spec or running binary as input). Schema-driven generation commands — output is source code, not a report. `--json` is available but would wrap generated code in a string field, which is not useful for agents that want to write the output to a file anyway.

**Recommendation:** Confirm `generate cli-snapshot --json` returns `{code: string, path: string|null}` so agents can capture generated code programmatically. Not evaluated due to input requirements.

---

### `package list`

**Compact:** Prints `N dependencies (ecosystem)` with zero rows when the workspace root's virtual manifest has no direct deps. The note/hint about multiple ecosystems prints to stdout as a `note:` / `hint:` prefix line mixed with output.

**JSON:** `{"ecosystem": "cargo", "packages": []}` — clean but empty in the workspace-root case. The `note: multiple ecosystems detected` advisory appears in compact but not JSON, meaning JSON consumers silently get results for only one ecosystem when `--ecosystem` is omitted.

**Rating: MIXED.** The multi-ecosystem advisory disappears in `--json` mode. An agent using JSON to list all dependencies on a polyglot repo would get incomplete results with no signal that other ecosystems exist. The advisory should appear as a field in the JSON response (e.g., `"other_ecosystems": ["npm", "nix"]`).

**Issue (High):** `--json` silently drops the multi-ecosystem advisory. Agent receives partial results with no indication other ecosystems were skipped.

---

### `package tree`

**Compact:** Indented tree with version suffixes — readable but requires custom parsing for deep trees. Depth is unbounded (full transitive closure); the output for a real workspace can be hundreds of KB.

**JSON:** Recursive `{name, version, dependencies: [...]}` tree — structurally correct but massive for large dependency graphs (122KB+ observed). `--jsonl` is not useful here since the tree is one object.

**Rating: MIXED.** Output size is the main issue. No `--depth` flag to limit recursion for agents that only need the direct-dependency layer.

**Issue (Medium):** No `--depth N` flag. Agents wanting only direct dependencies receive the entire transitive closure. Add `--depth 1` for direct-only.

---

### `package why <name>`

**Compact:** `'X' is required by N path(s):\n` then one tree path per paragraph — paths are indented dependency chains. The N count appears in the header but the actual paths are multi-line paragraphs with no separator marker.

**JSON:** `{"ecosystem": "...", "package": "serde", "paths": [[{name, version}, ...]]}` — array of arrays. Clean and correct; each path is an ordered list of `{name, version}` objects from root to the queried package.

**Rating: GOOD.** JSON is agent-friendly. Compact is verbose for packages with many paths (serde has 71 paths, generating ~100 lines).

---

### `package info <name>`

**Compact:** Multi-line block: name + version on line 1, description on line 2, blank line, `license:`, `homepage:`, `repository:`, blank line, `features:` table. Human-readable block format with no field separators.

**JSON:** `{"ecosystem": "...", "info": {name, version, description, license, homepage, repository, features: [{name, dependencies, description}]}}` — fully structured, features include dependency arrays.

**Rating: GOOD.** JSON is clean. Compact is verbose but readable.

---

### `package outdated`

**Compact:** `All packages are up to date` when nothing is outdated — a single clean line. When packages are outdated, the format is unknown (not tested with actual outdated packages).

**JSON:** `{"ecosystem": "...", "outdated": [], "errors": []}` — `errors` array is useful; reports packages where version resolution failed.

**Rating: GOOD** for the all-current case. **Untested** for the outdated case.

---

### `package audit`

**Compact:** `No vulnerabilities found (ecosystem).` — single line.

**JSON:** `{"ecosystem": "...", "vulnerabilities": []}` — clean.

**Rating: GOOD** for the no-vulnerabilities case.

---

### `tools lint list` / `tools test list`

**Compact (lint):** Header `Detected tools:` then indented blocks per tool — checkmark, name, category, version string, extensions, website. Multi-line per tool with no machine-parseable separator.

**JSON (lint):** `{"tools": [{available, category, extensions, name, version, website}]}` — well-structured. `available: true/false` is the key field.

**Compact (test):** Aligned table with columns: name, description, `(detected)` or `(not installed)`. Clean two-state format.

**JSON (test):** `{"runners": [{available, description, detected, name}]}` — `detected` (found in manifest) vs `available` (binary on PATH) are separate booleans, which is important: a runner can be `detected: true` (in package.json) but `available: false` (bun not on PATH).

**Rating: GOOD for JSON.** Compact lint output is multi-line blocks requiring custom parsing; compact test output is a clean table. The `detected` vs `available` distinction in test JSON is semantically important and well-modeled.

---

### `edit history list`

**Compact:** Numbered entries: `N. [HEAD] operation: target in file1, file2, ...` — the file list is a comma-separated string that can be hundreds of characters for bulk edits. No line wrapping; one line per entry.

**JSON:** `{"checkpoint": "sha", "head": N, "edits": [{id, operation, target, files: [...], timestamp, git_head, hash, message, subject, workflow}]}` — complete and well-structured. `files` is a proper array.

**Rating: MIXED.** JSON is GOOD. Compact file-list runs to 2000+ characters for bulk `--each` operations (observed: 100+ files in one edit record). An agent reading compact history to find recent edits cannot meaningfully parse the file list without string splitting on `, `.

**Issue (Low):** Compact history entry file lists should be truncated with a count (`+N more`) similar to how `aliases` truncates patterns. Agents that need the full file list should use `--json`.

---

### `budget measure`

**Compact:** `path  metric=X aggregate=Y ref=REF\n  added=N removed=N total=N net=N (N items)` — two-line format with labeled values. Consistent and parseable.

**JSON:** `{"path", "metric", "aggregate", "ref", "added", "removed", "total", "net", "item_count"}` — complete scalar fields, no nested objects.

**Rating: GOOD.**

---

### `budget show`

**Compact:** `budget: no matching entries` when empty. Clean.

**JSON:** `{"entries": []}` — clean.

**Rating: GOOD.**

---

### `ratchet measure`

**Compact:** `path  metric=X aggregate=Y value=N.NNNN (N items)` — single line, labeled. Consistent with `budget measure` format.

**JSON:** `{"path", "metric", "aggregate", "value", "item_count"}` — clean scalars.

**Rating: GOOD.** The compact and JSON formats are the most machine-friendly in the CLI surface — simple labeled scalars with no markdown or nesting.

---

### `ratchet show`

**Compact:** `No entries found.` when empty.

**JSON:** `{"entries": []}`.

**Rating: GOOD.**

---

### `config show`

**Compact:** Emits the config file as annotated TOML with `#` comment blocks explaining each field — essentially a config file template. The output format is TOML, not a summary table. Full config including all defaults.

**`--set-only` variant:** Same annotated TOML but shows only sections that have values set — shorter.

**JSON:** `{"config_path", "content": <full config object>, "section": null}` — the `content` field is the complete parsed config as a JSON object. Fully structured; agents can navigate to any field with `--jq`.

**Rating: GOOD.** `--json` is the right tool for agents. The annotated TOML compact output is intentionally human-oriented (a config template).

---

### `config validate`

**Compact:** `✓ Config is valid: path/to/config.toml` — single line with a Unicode checkmark. The checkmark is not machine-parseable without Unicode support; exit code carries the actual signal.

**JSON:** `{"config_path", "valid": true, "errors": [], "schema_source", "warnings": []}` — Boolean `valid` with `errors` and `warnings` arrays. Exactly what an agent needs.

**Rating: GOOD for JSON.** Compact uses a Unicode symbol for success state — exit code is the reliable signal. No issues.

---

### `config set`

Not evaluated (mutating; requires a key=value argument). `--dry-run` is not available. Agents should use `config validate` after `config set` to confirm correctness.

---

### `ci`

**Compact:** `N issues (N files)\n  N warnings, N info\n` header, then one `path:line: severity [rule-id] message` line per issue. Compact and dense — the `[rule-id]` in brackets is parseable via regex. Summary line before issues gives totals upfront.

**JSON:** `{"diagnostics": {files_checked, issues: [{file, line, column, end_line, end_column, rule_id, severity, message, source}]}, "engines_run": [...], "duration_ms", "error_count", "warning_count", "info_count"}` — complete structured output. `source` field (e.g., `"syntax-rules"`) tells which engine fired the issue.

**SARIF mode:** `--sarif` available for GitHub Actions annotations.

**Rating: GOOD.** The per-issue compact format is agent-parseable (consistent `path:line: severity [rule-id] message` pattern). JSON is complete. `--sarif` is a notable affordance for CI pipelines.

**Minor issue:** The compact summary line says `"N issues (N files)"` — the "files" count is the number of files **checked** (1923 in testing), not the number of files with issues. An agent reading the summary to gauge scope might misinterpret this as "N files have issues."

---

### `daemon status`

**Compact:** `Daemon is not running\nSocket: /path/to/socket` or running equivalent — two lines.

**JSON:** `{"running": false, "socket": "/path/..."}` — minimal, correct.

**Rating: GOOD.**

**Issue:** `daemon list` (list watched roots) returns exit code 1 with `"Daemon is not running"` stderr when the daemon is stopped — no stdout JSON. An agent that polls `daemon list --json` to check watched roots gets an error exit code with no structured response body. Should return `{"running": false, "roots": []}` (exit 0) instead of failing.

---

### `analyze health`

**Compact:** Markdown-formatted output with `#` and `##` headers, score letters (grade `D`), percentage scores, and bullet-style lines. The grade (`D (69%)`) is a human-readable summary; no machine-readable version in compact.

**JSON:** Returns the full health object with all raw scores. Very large (180KB+ for the normalize repo due to the `large_files` array containing every oversized file). The `large_files` array is unbounded — it includes every large file in the codebase, which for a repo with many worktrees becomes massive.

**Rating: MIXED.** JSON is **POOR for large repos** — the `large_files` array bloats the response to unusable size. An agent asking "what is the overall health score?" receives a 180KB response to get three numbers. A `--summary` flag or default truncation of `large_files` would fix this.

**Issue (High):** `analyze health --json` is unbounded in size. Add `--limit N` on `large_files` (or default to top-10) to make the JSON response usable for agents.

---

### `analyze summary`

**Compact:** 5–6 line markdown summary: grade, file count, line count, language count, function count, composition breakdown, health scores, top concerns, and architecture count. Compact and information-dense.

**JSON:** Same structure as `analyze health` JSON but even larger (182KB+ due to full file lists). Like `health`, the response includes the full `large_files` array.

**Rating: MIXED.** Compact output is the best summary format in the CLI surface — concise and parseable. JSON suffers from the same unbounded-size issue as `analyze health`. **Recommended pattern for agents: use `analyze summary --compact` for a token-efficient codebase overview.**

---

### `analyze security`

**Compact:** `# Security Analysis\n\nFindings: N critical, N high, N medium, N low\nTools skipped (not installed): bandit` — two lines of content with a markdown `#` header and prose.

**JSON:** `{"findings": [], "tools_run": [], "tools_skipped": ["bandit"]}` — clean. `tools_skipped` tells agents which scanners were absent.

**Rating: GOOD for JSON.** Compact markdown header is noise.

---

### `analyze docs`

**Compact:** `# Documentation Coverage — N% (N of N documented)\n\n## By Language\n...table...## Worst Coverage\n...table...` — markdown headers with columnar tables. Column widths vary with filename length.

**JSON:** `{"coverage_percent", "documented", "total_callables", "by_language": {lang: [documented, total]}, "worst_files": [{file_path, documented, total}]}` — clean and complete.

**Rating: GOOD for JSON.** Compact uses markdown headers that an agent must strip.

**Issue:** `by_language` in JSON uses a `{lang: [N, N]}` array encoding instead of `{lang: {documented: N, total: N}}` — the two-element array is positional and undocumented in the schema. An agent reading `by_language.Rust[0]` vs `[1]` must guess which is documented vs total.

---

### `analyze skeleton-diff`

**Compact:** `# Skeleton Diff (vs SHA)\nN files changed: +N symbols, -N symbols, ~N changed\n` header, then per-file lines with `M/A/D` change markers and `+N -N ~N` counters, followed by indented symbol change lines. Clean diff format.

**JSON:** `{"base_ref", "changes": [{name, kind, path, change, before_signature, after_signature}], "files": [{path, status, symbols_added, symbols_changed, symbols_removed}], "total_added", "total_changed", "total_removed"}` — complete with full before/after signatures for changed symbols.

**Rating: GOOD.** One of the best-designed outputs in the surface. The `change` field enumerates the change type (`"signature_changed"`, `"added"`, `"removed"`), and `before_signature` / `after_signature` give agents the full context to understand what changed without reading source code.

---

### `analyze architecture`

**Compact:** Prints only `## Cross-Imports` section by default — a list of bidirectional coupling pairs. Missing the coupling hotspots and hub modules sections visible in JSON.

**JSON:** `{cross_imports, coupling_hotspots, hub_modules, orphan_modules, symbol_hotspots, layer_flows, ...}` — comprehensive architectural analysis. Several sections (hub modules, symbol hotspots, layer flows) are not represented at all in compact output.

**Rating: POOR for compact / GOOD for JSON.** The compact output is a subset of what JSON returns — coupling pairs only. Agents using compact miss hub modules and layer flows entirely. An agent that reads compact to find the most problematic file in the codebase will get cross-import pairs but not the `coupling_hotspots` ranking.

**Issue (Medium):** Compact output omits hub module ranking and layer flow analysis. These are the most actionable outputs. Add compact-friendly sections for `coupling_hotspots` (top-N by hub score) and `hub_modules`.

---

### `analyze length`

**Compact:** Markdown-formatted with `# Function Length Analysis` header, scalar stats, `## Longest Functions` with `### Too Long` / `### Long` subsections, then `N lines path:symbol` entries. Multi-level markdown headers.

**JSON:** `{"total_count", "total_avg", "total_max", "critical_count", "high_count", "functions": [{name, parent, file_path, start_line, end_line, lines}]}` — complete and well-structured.

**Rating: MIXED.** JSON is GOOD. Compact markdown headers add noise. The entry format `172 path:symbol` is parseable.

---

### `analyze test-gaps`

**Compact:** Header with total counts, then aligned table: `Risk  Function  File  Complexity  Callers  LOC`. File column is truncated to a fixed width — agents cannot reconstruct the full path.

**JSON:** `{total_public, untested_count, allowed_count, show_all, functions: [{name, parent, file_path, start_line, end_line, complexity, caller_count, test_caller_count, loc, risk, de_prioritized}]}` — complete, full paths included.

**Rating: MIXED.** JSON is GOOD. Compact truncates file paths — agents must use `--json` to get actionable file locations. The `de_prioritized` flag in JSON (whether the function is in an allowlist) is absent from compact output.

---

### `syntax ast`

**Compact/Pretty:** Renders the CST as indented JSON-like text (actually JSON formatted with indentation). The compact flag has no effect — output is always the indented tree.

**JSON:** The same tree as a JSON object — but for a real file the output is enormous (673KB+ for a 200-line Rust file). Every token is a node with `{kind, start: {row, col}, end: {row, col}, text, children}`. Text is included inline for leaf nodes.

**Rating: POOR for large files** due to output size. Practical for small files or `--at-line N` scoped to a single node. The `--at-line` flag is essential for agent use — always use it with a specific line.

**Issue (High):** No `--depth N` flag to limit tree depth. The output for a 200-line file is 673KB. An agent wanting to understand the top-level structure of a file must receive the full deep tree. Add `--depth N` (equivalent to the guide examples that reference it, which are apparently wrong about flag name).

**Note:** The guide (`guide rules`) references `normalize syntax ast file.py --depth 3` but this flag does not exist. The `--at-line` flag scopes to a node but does not limit depth. This is a documentation bug.

---

### `syntax query`

**Compact:** Prints only `N matches` — the match count with no detail. The actual match content (file, line, capture text) is absent from compact output.

**JSON:** `[{file, grammar, kind, start_row, start_col, end_row, end_col, text, captures: {name: text}}]` — complete per-match data.

**Rating: POOR for compact.** `--compact` suppresses all match detail, returning only a count. An agent using `syntax query` to find specific code patterns gets no usable data in compact mode. The `--json` output is well-structured and the correct tool for agent use.

**Issue (High):** Compact output for `syntax query` is useless beyond counting — it omits all match location and capture data. At minimum, compact should emit one `file:line: capture_text` line per match, matching the format of `rules run` output.

---

### `rules show`

**Compact:** Full rule documentation including description, fix guidance, disable rationale, and TOML snippet — appropriate for human consumption.

**JSON:** `{"message": "<full compact text as a string>", "success": true}` — the entire human-readable text is wrapped in a `message` field string. The structured data (severity, enabled status, tags, allow list) that appears in compact text is **not broken out into JSON fields**. An agent wanting to check whether a rule is enabled must parse the `message` string.

**Rating: POOR for JSON.** The `--output-schema` confirms `RuleShowReport {message: string, success: bool}` — the output type is a raw text wrapper. Contrast with `rules list --json` which returns proper structured `RuleEntry` objects per rule. `rules show --json` should return a structured object with `{id, severity, enabled, tags, languages, allow, message, fix_guidance}` instead of a text blob.

**Issue (High):** `rules show --json` returns `{message: "<human text>", success: bool}` — identical to compact wrapped in JSON. Agents cannot read rule metadata programmatically. Should return structured fields matching `RuleEntry` from `rules list` plus the full description.

---

### `rules tags`

**Compact:** `tag  [builtin]  N rules` — clean aligned table.

**JSON:** `{"message": "<compact text as a string>", "success": true}` — same `RuleShowReport` wrapper as `rules show`. The tag-to-count mapping is inside the message string, not structured fields.

**Rating: POOR for JSON** — same defect as `rules show`. The JSON response for `rules tags` is the compact text wrapped in a string. An agent asking "how many rules are in the cleanup tag?" must parse `"cleanup              [builtin]  22 rules"` from a string.

**Issue (High):** Same as `rules show` — `rules tags --json` returns unstructured text. Should return `{"tags": [{name, source, rule_count, rules: [...]}]}`.

---

### `rules validate`

**Compact:** `Rules configuration is valid\n\nConfig file: path\n  N rule overrides, N global-allow patterns` — success message with summary counts.

**JSON:** `{"config_path", "valid": true, "errors": [], "warnings": [], "rule_count", "global_allow_count"}` — clean, structured, with separate `errors` and `warnings` arrays.

**Rating: GOOD.**

---

## Pass 2 Prioritized Issues

Issues ordered by agent impact.

| # | Issue | Command | Severity |
|---|-------|---------|----------|
| 1 | ~~`rules show --json` and `rules tags --json` return human text wrapped in `{message: string}` instead of structured fields~~ **Fixed 2026-03-26** — `rules show` now returns `RuleInfoReport`, `rules tags` returns `RulesTagsReport` | `rules show`, `rules tags` | **High** — agents cannot read rule metadata programmatically |
| 2 | `syntax query --compact` returns only a match count — all match data suppressed | `syntax query` | **High** — compact output is useless for pattern search |
| 3 | `syntax ast --json` for a real file is 500KB+; no `--depth N` flag | `syntax ast` | **High** — output size makes it impractical for agent use; guide doc references `--depth` flag that doesn't exist |
| 4 | `analyze health --json` and `analyze summary --json` are 180KB+ due to unbounded `large_files` array | `analyze health`, `analyze summary` | **High** — JSON response too large for practical agent consumption |
| 5 | `package list --json` silently drops multi-ecosystem advisory; agent gets partial results | `package list` | **High** — silent data loss |
| 6 | `analyze architecture --compact` omits hub modules and layer flows; compact is a subset of JSON | `analyze architecture` | **Medium** — agents using compact miss the most actionable outputs |
| 7 | ~~`daemon list` returns exit 1 + stderr when daemon is stopped instead of `{"running": false, "roots": []}`~~ **Fixed 2026-03-26** — now returns exit 0 with `{running: false, roots: []}` when daemon is not running | `daemon list` | **Medium** — non-zero exit prevents agents from using `list` as a status check |
| 8 | `package tree --json` has no `--depth` flag; full transitive graph is 122KB+ | `package tree` | **Medium** — agents wanting direct-deps-only receive the full closure |
| 9 | `grammars list --json` has no path per grammar; two calls needed to confirm a grammar is loadable | `grammars list` | **Low** — inconvenient but workaround (grammars paths) exists |
| 10 | `analyze docs` JSON encodes `by_language` as positional array `[documented, total]` instead of named fields | `analyze docs` | **Low** — positional arrays require documentation to decode |
| 11 | `context` compact output has no per-file delimiter in multi-file repos | `context` | **Low** — only affects repos with multiple `.context.md` files |
| 12 | `ci` compact summary says "N files" meaning files-checked, not files-with-issues | `ci` | **Low** — potential misread of scope |

## Pass 2 Per-Command Consensus

| Command | Rating | Key Issues |
|---------|--------|-----------|
| `aliases` | **MIXED** | `+N` truncation in compact loses patterns |
| `context` | **MIXED** | No per-file delimiter in multi-file compact output |
| `init` | **GOOD** | Clean JSON |
| `update` | **GOOD** | Clean JSON with `update_available` boolean |
| `grammars list` | **GOOD** | No path per grammar in JSON |
| `grammars paths` | **GOOD** | `exists` boolean is well-modeled |
| `guide` | **POOR** | Prose output by design; not machine-parseable |
| `package list` | **MIXED** | Multi-ecosystem advisory lost in JSON |
| `package tree` | **MIXED** | Unbounded size; no `--depth` |
| `package why` | **GOOD** | Clean JSON paths array |
| `package info` | **GOOD** | Structured JSON |
| `package outdated` | **GOOD** | Clean boolean result |
| `package audit` | **GOOD** | Clean JSON |
| `tools lint list` | **GOOD** | `available` boolean useful |
| `tools test list` | **GOOD** | `detected` vs `available` distinction well-modeled |
| `edit history list` | **MIXED** | File lists become thousands of chars in compact for bulk ops |
| `budget measure` | **GOOD** | Clean labeled scalars |
| `budget show` | **GOOD** | Clean |
| `ratchet measure` | **GOOD** | Best compact format in the surface |
| `ratchet show` | **GOOD** | Clean |
| `config show` | **GOOD** | Annotated TOML in compact, full object in JSON |
| `config validate` | **GOOD** | Boolean + arrays |
| `ci` | **GOOD** | `source` field useful; SARIF support notable |
| `daemon status` | **GOOD** | Clean |
| `daemon list` | **GOOD** (fixed 2026-03-26) | Exit 0 with `{running: false, roots: []}` when daemon stopped |
| `analyze health` | **MIXED** | JSON too large for agent use |
| `analyze summary` | **MIXED** | Best compact format; JSON too large |
| `analyze security` | **GOOD** | `tools_skipped` field useful |
| `analyze docs` | **GOOD** | Positional array in `by_language` is minor issue |
| `analyze skeleton-diff` | **GOOD** | Best-designed output with before/after signatures |
| `analyze architecture` | **POOR** compact / **GOOD** JSON | Compact omits hub modules and layer flows |
| `analyze length` | **MIXED** | Markdown headers in compact; JSON is clean |
| `analyze test-gaps` | **MIXED** | File paths truncated in compact |
| `syntax ast` | **POOR** | 500KB+ JSON for real files; no `--depth` flag |
| `syntax query` | **POOR** compact | Compact returns count only; JSON is well-structured |
| `rules show` | **GOOD** JSON (fixed 2026-03-26) | Returns structured `RuleInfoReport` with id, severity, enabled, tags, languages, message, fix, description, allow |
| `rules tags` | **GOOD** JSON (fixed 2026-03-26) | Returns `RulesTagsReport` with `{tags: [{tag, source, count, rules}]}` |
| `rules validate` | **GOOD** | Clean structured JSON |

## Pass 2 Quick Wins

1. **`rules show --json` and `rules tags --json`** — change return type from `RuleShowReport {message}` to structured fields. Both commands have clean structured data available (already in `rules list`); the JSON wrapper is the only obstacle.
2. **`syntax query --compact`** — emit `file:line: capture_text` lines instead of count-only. One-line change that makes compact useful for agents.
3. **`package list --json` multi-ecosystem advisory** — add `other_ecosystems: []` field to JSON response so agents know when results are partial.
4. **`daemon list` when daemon stopped** — return exit 0 with `{"running": false, "roots": []}` instead of exit 1 + stderr.
5. **`analyze health / summary --json` size** — default `large_files` to top-10 (add `--limit` to override). Reduces response from 180KB to ~5KB for the common case.

## Pass 2 Meta-Observation

The structural commands (`ratchet measure`, `budget measure`, `config validate`) are the best-designed in the surface: single-line compact with labeled scalars, clean JSON with scalar fields, exit code carries the error signal. These are the templates to follow.

The worst pattern is `RuleShowReport {message: string}` — a text wrapper that provides the appearance of JSON without any of the machine-readability. Three commands (`rules show`, `rules tags`, and implicitly any future "show" command that reuses this type) are affected. The fix is a one-time type change.

Output size is the emerging problem for larger codebases: `syntax ast`, `analyze health`, `analyze summary`, and `package tree` all produce responses that exceed practical agent context window budgets. The fix pattern is `--depth N` or `--limit N` flags with sensible defaults.
