# CLI Audit 2026-06-29 — Consolidated Triage

Master de-duplicated catalogue synthesizing the five read-only audits in this directory:

- `01-structured-output.md` — `--json`/`--jsonl` coverage across 103 commands
- `02-flag-naming.md` — flag-name consistency + the 0.6 `#[param(name)]` rename
- `03-dry-run.md` — `--dry-run` coverage on mutating commands
- `04-errors-exit-codes.md` — exit codes + error-channel behavior
- `05-command-structure.md` — command-tree shape, duplicates, stale guides

Findings that appeared in multiple audits are merged into a single entry below (provenance
noted per entry). Cross-references to items already tracked in `TODO.md` are flagged so we
don't double-book work.

**Legend:** Sev = HIGH/MED/LOW · HC = CLAUDE.md hard-constraint violation · Owner = normalize
| server-less · Effort = S (<½day) / M (1–2 days) / L (multi-day).

---

## Tier 1 — Hard-constraint violations + actively broken

| ID | Title | Sev | HC | Owner | Effort |
|----|-------|-----|----|----|--------|
| T1-1 | Index-dependent commands exit 0 + empty JSON on missing index | HIGH | **YES** | normalize | M |
| T1-2 | 22 mutating commands ship without `--dry-run` | HIGH | **YES** | normalize | L |
| T1-3 | All `trend` metric commands broken (stale worktree `[embeddings]` config) | HIGH | — | normalize | S–M |
| T1-4 | `structure packages` silently succeeds with zero output | HIGH | **YES** (silent empty) | normalize | S |
| T1-5 | `rules show <id>` lookup bug — rule in `list`, not found by `show` | HIGH | — | normalize | S |
| T1-6 | Broken guides + stale `kg --help` examples reference nonexistent commands | HIGH | — | normalize | S–M |

### T1-1 — Index-dependent commands exit 0 + empty JSON on missing index
*Merged from: 04 HIGH-1..6, 01 (`analyze coupling-clusters` empty), 04 MED-3.*

When the import/call-graph index has not been built, `view graph`, `view dependents`,
`view import-path`, `rank imports`, `rank depth-map`, `rank layering`, `rank call-complexity`,
and `analyze architecture` all **exit 0** and return empty/zeroed collections. Text mode usually
prints an advisory ("Run `normalize structure rebuild` first") but JSON mode emits a bare empty
result with no signal — indistinguishable from "this codebase genuinely has no imports."

Evidence:
```
$ normalize view graph --json        # no index
{"stats":{"nodes":0,"edges":0,...}}  exit 0
$ normalize rank imports --json       # no index
{"entries":[],"total_imports":0,...}  exit 0
```

**Hard-constraint violation.** CLAUDE.md (LLM-Driven Workflows): *"Non-interactive ≠
non-functional… print a clear actionable message to stderr and exit with a non-zero code. Never
silently return empty results."* Fix: when the prerequisite table is empty but source files exist,
exit non-zero and include `"requires_index": true` (or an `"error"` field) in JSON output.

### T1-2 — 22 mutating commands ship without `--dry-run`
*From: 03 (whole audit).*

CLAUDE.md Hard Constraints forbid shipping mutating commands without `--dry-run`. 22 violate this:
`edit redo`, `rules run --fix`, `rules add/update/remove/setup`, `structure rebuild`,
`structure packages`, `kg write`, `sessions mark/unmark`, `update`, `ratchet add/update/remove`,
`budget add/update/remove`, `daemon start/stop`, `generate client`, `generate cli-snapshot`.

**Hard-constraint violation.** Priority order within this item (highest blast radius first):
1. `edit redo` — asymmetric with `edit undo` which *has* `--dry-run`; one-line fix (source confirms `redo()` hardcodes `dry_run: false`).
2. `rules run --fix` — applies fixes across the whole tree, no preview.
3. `kg write` with a `null` transform — destructive delete, no confirmation.
4. `rules add/update/remove`, then `ratchet`/`budget` CRUD, then `sessions mark/unmark`.

Borderline (still violations, lower urgency): `daemon start/stop` (process control),
`update` (`--check` covers most of the need), `generate client/cli-snapshot` (stdout-by-default
is a de-facto dry-run). Even idempotent maintenance ops (`structure rebuild/packages`) are not
carved out by the constraint as written — either add `--dry-run` or amend the constraint to
exempt idempotent rebuilds (decision needed).

### T1-3 — All `trend` metric commands broken (stale worktree config)
*From: 01 HIGH.*

```
$ normalize trend complexity --json
error: /tmp/normalize-wt-6c37ecd/.normalize/config.toml contains [embeddings]
which was removed in 0.3.0...
```
`trend complexity/length/density/test-ratio` all fail. They spawn a temp git worktree to check
out historical commits; that worktree's `.normalize/config.toml` carries a deprecated
`[embeddings]` section and config validation hard-aborts. All four metric trends are currently a
full outage. Fix options: (a) ignore unknown config sections when running inside a normalize-spawned
worktree, (b) pass an `--ignore-config`-equivalent in that path, or (c) don't materialize a config
in the throwaway worktree at all. **Confirm the root cause is the spawn path, not a leftover on this
machine — if it's machine-local stale state, the bug is that validation aborts on a section we
ourselves wrote into a throwaway tree.**

### T1-4 — `structure packages` silently succeeds with zero output
*Merged from: 01 HIGH, 03 (also a missing-`--dry-run` case).*

`normalize structure packages --json` produces no stdout, no stderr, exit 0. Violates "never
silently return empty results." Should emit at minimum `{"indexed":0,"ecosystems":[]}` and, since
it writes to the global grammar cache, also needs `--dry-run` (counts toward T1-2).

### T1-5 — `rules show <id>` lookup bug
*From: 01 HIGH.*

`rules show stale-summary` → `Rule not found: stale-summary`, yet `stale-summary` appears in
`rules list` output and `rules show barrel-file` works. Likely a registry split (native vs fact
rules) that `show` doesn't search. The error is also plain text rather than JSON (see T2-2).

### T1-6 — Broken guides + stale `kg --help` examples
*Merged from: 05 H-3, H-4, H-5.*

- `guide analyze` shows ~7 commands moved from `analyze` → `rank` long ago (`analyze complexity`,
  `analyze length`, `analyze duplicates`, …) — following the guide yields "unrecognized subcommand".
- `guide rules` references `analyze node-types` (now `syntax node-types`).
- `kg --help` epilogue references `kg create/link/query/show` — none exist (actual: `read/write/walk`).

These are user-facing-wrong on first contact. Fix the stale strings and add a snapshot/`guide test`
that parses guide bodies against the real command tree so this can't regress (the guide system has
no test coverage, unlike `rules test-fixtures` / `structure test-fixtures`).

---

## Tier 2 — Correctness / consistency

| ID | Title | Sev | Owner | Effort |
|----|-------|-----|----|--------|
| T2-1 | `--jsonl` does not unwrap inner arrays (object-wrapping reports) | MED | normalize | M |
| T2-2 | Service-layer errors emitted as plain text even with `--json` | MED | normalize | M |
| T2-3 | `budget` `--base-ref`→`--diff-ref` silent rename + git-ref flag taxonomy | MED | normalize | S (decide) / M (unify) |
| T2-4 | `--ignore-case` (grep) vs `--case-insensitive` (view family) | MED | normalize | S |
| T2-5 | `--limit` short-form `-l` vs `-n`; `--n`; `--top`/`--worst` aliases | MED | normalize | M |
| T2-6 | `rank budget` name collision with `budget` service | MED | normalize | S |
| T2-7 | `cfg cfg` redundant double-wrapping | MED | normalize | S |
| T2-8 | Git ref as positional in `view trace --target` / `skeleton-diff [base]` | MED | normalize | S |
| T2-9 | Inverted dry-run default: `edit extract-function`, `context migrate` | MED | normalize | S |
| T2-10 | `edit history` vs `view history` — same name, different concept | MED | normalize | S |

### T2-1 — `--jsonl` does not unwrap inner arrays
*From: 01.* `structure files`, `rank complexity`, `rules list`, `grep` all emit the wrapper object
`{"items":[...]}` as a single `--jsonl` line — identical to `--json`, useless for line-streaming
consumers. `view list` is the only command that correctly emits one object per line (it returns a
bare array). Fix: list-wrapping reports implement a `jsonl_items()` hook to unwrap.

### T2-2 — Service-layer errors are plain text under `--json`
*Merged from: 01 MED block, 01 (view trace HIGH), 04.* `docs`, `view references`,
`view referenced-by`, `serve mcp`, `view trace`, and missing-required-arg errors all print plain
text even when `--json` is active. `--json` consumers should get a JSON error object. Clap-level
validation errors (before the service runs) are lower priority and partly a server-less concern;
service-layer errors are squarely normalize's. `view trace` is the sharpest case: it exits 1 with
`Trace failed with exit code 1` and no JSON.

### T2-3 — `budget` `--base-ref`→`--diff-ref` rename (consequence of our own 0.6 work) + ref-flag taxonomy
*Merged from: 02 §1, 02 §2.2. **Cross-references TODO.md** "Audit normalize for `#[param(name)]`…".*

This is the one finding caused directly by our recent server-less 0.6 adoption. server-less 0.6
began honoring `#[param(name = "...")]`; 0.5.x silently ignored it. The only two such annotations in
the workspace are both in `normalize-budget/src/service.rs` (`measure`, `add`):
```rust
#[param(name = "diff-ref", help = "Compute diff against this git ref")] base_ref: Option<String>
```
So the flag changed `--base-ref` (0.5.x, derived from the `base_ref` field) → `--diff-ref` (0.6.x,
honoring the attribute). **See verdict below — recommendation: keep the rename.**

Broader (separable) issue: five spellings exist for "compare against a git ref" — `--diff`
(all `rank`), `--diff-ref` (budget/ratchet measure), `--baseline-ref` (ratchet check),
positional `[base]` (skeleton-diff). Worth a deliberate taxonomy pass, but not urgent.

### T2-4 — `--ignore-case` vs `--case-insensitive`
*From: 02 §2.1.* `grep` uses `-i, --ignore-case` (rg/git-grep convention); the entire `view` family
uses `-i, --case-insensitive`. Canonicalize on `--ignore-case` everywhere.

### T2-5 — `--limit` short-form and alias drift
*Merged from: 02 §2.3, §2.4, §2.5, §4.3, §4.5, §4.6.* Most commands use `-l, --limit`; the `sessions`
family uses `-n, --limit`. `sessions ngrams` has a long-only `--n` (ngram size) that looks like a
typo because `-n` is taken. `--top` (`sessions ngrams/heatmap`) and `--worst` (`rank density`) are
aliases for the same "cap N results" concept. `syntax ast` rebinds `-l` to `--at-line`. Canonicalize
on `-l, --limit`; rename ngram size to `--ngram`/`--size`; retire `--top`/`--worst`.

### T2-6 — `rank budget` collides with `budget` service
*From: 05 H-2.* `rank budget` (line-count breakdown by purpose) and the top-level `budget` service
(PR diff-size CRUD) are unrelated. Rename `rank budget` → `rank line-breakdown` / `rank purposes`.

### T2-7 — `cfg cfg` redundant double-wrapping
*From: 05 H-1.* `cfg` is a service with one subcommand also named `cfg`; the only invocation is
`normalize cfg cfg <path>`. Collapse to a direct `normalize cfg <path>` command.

### T2-8 — Git ref / target as positional vs flag
*Merged from: 02 §3.1, §3.2.* `view trace` splits target into `[symbol]` positional + `--target`
flag while every other `view` subcommand takes a single `path/symbol` positional;
`analyze skeleton-diff` takes the ref as positional `[base]` while everything else uses a flag.
Align both with the prevailing form.

### T2-9 — Inverted dry-run default
*Merged from: 02 §3.3, 05 M-2, 03 notes.* `edit extract-function` and `context migrate` default to
dry-run and require `--apply`, inverting the write-by-default + `--dry-run` convention of every other
`edit` command. Note: these are **not** T1-2 violations (they *have* a preview path) — this is a
consistency defect. Decision: either flip them to write-by-default + `--dry-run`, or keep
`extract-function` as a documented deliberate-safety exception (it's the most dangerous edit recipe).
Recommend documenting the exception rather than removing the guard.

### T2-10 — `edit history` vs `view history`
*From: 05 M-3.* `edit history` is the shadow-git undo log; `view history` is git commit history. CLI
convention reads "history" as git history. Rename `edit history` → `edit log`/`edit trail`.

---

## Tier 3 — Polish (help text, noise, examples, soft overlaps)

| ID | Title | Sev | Owner | Effort |
|----|-------|-----|----|--------|
| T3-1 | Root-global flag noise (~9 flags on every command's `--help`) | LOW | server-less | M |
| T3-2 | Help-text gaps: missing examples / body paragraphs | LOW | normalize | S |
| T3-3 | Short-flag overloading + truncated/clashing flag descriptions | LOW | normalize / server-less | S |
| T3-4 | Soft feature overlaps without cross-references | LOW | normalize | S |
| T3-5 | Unactionable errors: `grep <nonexistent path>`, `analyze complexity` | LOW | normalize | S |

### T3-1 — Root-global flag noise
*Merged from: 05 (Root-Global Flag Noise), **already tracked in TODO.md** ("Root-global `--pretty`
advertised-no-op").* Every one of ~165 leaf commands renders the same 9 server-less globals
(`--pretty --compact --json --jsonl --jq --input-schema --output-schema --manual --params-json`),
often more lines than the command's own flags. This is a **server-less rendering concern**: collapse
into a `[global options]` footer or omit from leaf help with a pointer to `normalize --help`.
Related to (but broader than) the already-tracked advertised-no-op item — that one is about `--pretty`
being inert on reports with no `format_pretty`; this is about the visual noise of all nine globals.

### T3-2 — Help-text gaps
*From: 05 L-1..L-5, M-4.* `rules remove` / `rules update` have no examples; `analyze all` has no body
paragraph and an opaque scope (indistinguishable from `analyze health`); `analyze activity` /
`repo-coupling` / `cross-repo-health` lack examples; `analyze`'s category labels don't scan (no blank
lines). `syntax ast --compact` has a truncated description ("Enable compact").

### T3-3 — Short-flag overloading + flag-description bugs
*Merged from: 02 §4.1, §4.2, §4.4, 05 M-5.* `-d` means `--depth` / `--max-depth` / `--fixture-dir`;
`-t` means `--to`/`--type`/`--threshold`/`--target`. `syntax ast --compact` (AST outline mode)
clashes with the global `--compact` (no-color) — rename the AST flag to `--outline`. `view blame
--sessions` should be `--sessions-dir`.

### T3-4 — Soft feature overlaps
*Merged from: 05 M-1, M-6.* `analyze architecture` ≈ `view graph` (both: cycles + hubs);
`analyze coupling-clusters` ≈ `rank coupling` (same git co-change data, different aggregation).
Neither pair cross-references the other in help. Add "see also" lines, or merge.

### T3-5 — Unactionable errors
*From: 04 MED-1, MED-2.* `grep <pat> /nonexistent/path` silently reports "no matches" instead of
path-not-found. `analyze complexity` (a `rank` command) gives `path not found: complexity` with no
hint to try `rank complexity`.

---

## Verdict — the `--base-ref` → `--diff-ref` rename (T2-3)

**Intended and good. Keep `--diff-ref`. Do not revert.**

Reasoning:
1. **The author explicitly opted in.** The field is `base_ref` but the code carries
   `#[param(name = "diff-ref", help = "Compute diff against this git ref")]`. A `name` override is a
   deliberate act — nobody writes `name = "diff-ref"` by accident. The 0.5.x behavior (silently
   ignoring the attribute) was the bug; 0.6 honoring it makes the CLI finally match what the author
   wrote and what the help text already says ("Compute diff against this git ref" → `--diff-ref`).
2. **It improves consistency, not just intent.** `--diff-ref` aligns `budget`/`ratchet measure`
   (commands that take a *separate* ref to compute against) and reads better than `--base-ref` next
   to the `--diff` used by the `rank` family. The remaining taxonomy spread (`--diff` / `--diff-ref`
   / `--baseline-ref` / positional `[base]`) is a separate cleanup, not a reason to revert this one.
3. **But it is a real breaking change from published 0.5.x**, so it must be *handled*, not ignored:
   add a CHANGELOG entry under `[Unreleased]` documenting `budget measure`/`budget add`
   `--base-ref` → `--diff-ref`. Optionally accept `--base-ref` as a hidden alias for one release if
   any CI configs in the wild used it (low risk — only two methods, niche command).

Net: this is the system working as designed once 0.6 fixed the underlying server-less bug. Action is
documentation (CHANGELOG) + a one-line cross-reference to the existing TODO `#[param(name)]` audit
item, not code reversion.

---

## Bottom line

The CLI surface is broad (~30 services, ~165 leaf commands) and largely sound: ~65/103 commands
emit valid structured output, surface-level error contracts (bad flags, missing args, nonexistent
single-file paths) are clean, and the recently-fixed `--pretty` wiring holds. The damage is
concentrated and fixable. The genuinely urgent class is **silent-success-on-missing-prerequisite**
(T1-1, T1-4) and **mutation-without-preview** (T1-2) — both direct CLAUDE.md hard-constraint
violations that make the CLI unsafe for the agent/non-interactive use it's explicitly designed for —
plus one active outage (T1-3, all `trend` metrics) and a broken-on-first-contact guide/help layer
(T1-6). Tier 2 is mostly consistency debt accreted from commands migrating between `analyze`/`rank`
and from the 0.6 codegen change; none of it is broken, but it's the kind of drift that quietly
erodes the "API that happens to have a CLI" promise. Tier 3 is real but cosmetic, with the
root-global flag noise (T3-1) being the highest-leverage server-less-side cleanup. Recommended
sequencing: T1-3 (outage) and T1-1/T1-4 (HC + agent-blocking) first, then T1-2 starting with the
one-line `edit redo` fix, then T1-6; pull T2 items in opportunistically alongside the services they
touch.
