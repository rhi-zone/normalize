# CLI Design

> **Active restructuring: CLI taxonomy inversion (B0–B12).** The command tree below is being
> reorganized so each top-level verb *is* the crate that owns its `#[cli]` service (per
> CLAUDE.md's crate-owns-its-CLI rule). This is the **same operation** as the command-surface
> decomposition (reached from the size-reduction direction) — they are merged. Authoritative
> target taxonomy: `docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md` (FINAL SCOPE);
> reconciled roadmap + open forks: `docs/audit-2026-07-03-command-surface-decomposition.md`;
> batch tracking in `TODO.md`. Grab-bag `analyze`/`rank` shrink as crate-owned families
> (`graph`, `architecture`, `similarity`, `structure`, `history`, …) move out.

## Command Structure

`normalize --help` organizes commands into four tiered groups using `#[server(groups(...))]`. Core commands appear first; everything else is grouped by domain.

### Core
The essential daily-driver commands:
- `view` - View directory/file/symbol structure
- `grep` - Fast ripgrep-based text search
- `edit` - Structural code modifications (delete, replace, swap, insert, undo, redo, goto, batch, history)
- `rules` - Manage and run analysis rules (syntax + fact)
- `structure` - Build and query the structural index (rebuild, stats, files, packages, query, test-fixtures) plus CFG dataflow (liveness, effects, exceptions); owned by `normalize-facts`
- `init` - Initialize normalize in a directory

### Analysis
Assessment, metrics, and quality gates:
- `analyze` - Codebase analysis (per-metric + security/docs/skeleton-diff residual). Shrunk by inversion; the aggregate dashboards moved to `overview` (B11).
- `overview` - Codebase dashboards: `overview` (health), `overview --full` (all passes), `overview summary`, `overview cross-repo-health` (thin main-crate composition verb, B11). Was `analyze health`/`all`/`summary`/`cross-repo-health` — old paths kept as hidden aliases for one release. `cross-repo-health` lands here (main-resident composer, no cycle) rather than `history`.
- `graph` - Dependency-graph analysis: cycles/blast-radius/import-paths (`graph`, `graph dependents`, `graph import-path`; owned by `normalize-graph`, B2). Was `view graph`/`dependents`/`import-path` — old paths kept as hidden aliases for one release.
- `similarity` - Duplicate/near-duplicate code detection: clones, duplicate types, AST fragments (`similarity` incl. `--mode clusters`, `similarity duplicate-types`, `similarity fragments`; owned by `normalize-code-similarity`, B4). Index-free (walks the filesystem). Was `rank duplicates`/`duplicate-types`/`fragments` — old paths kept as hidden aliases for one release.
- `history` - Statistical code-health signals from git history: `hotspots`, `coupling`, `ownership`, `contributors`, `activity`, `repo-coupling`, `coupling-clusters` (owned by `normalize-git-history`, B9). Repo-wide cross-file analysis — distinct from `view history` (single-file git log). Was `rank hotspots`/`coupling`/`ownership`/`contributors` and `analyze activity`/`repo-coupling`/`coupling-clusters` — old paths kept as hidden aliases for one release. `cross-repo-health` went to `overview` instead (B11).
- `cfg` - Control-flow-graph render for a function (`normalize cfg <file>`; owned by `normalize-cfg`). The former redundant `cfg cfg` nesting was collapsed to a single leaf (B11).
- `rank` - Rank files/functions by metrics (permanent main-crate verb; A1). Includes `rank purposes` (line-purpose breakdown — was `rank budget`, renamed B11 to free `budget` for `normalize-budget`; hidden alias for one release).
- `trend` - Track metrics over git history (permanent main-crate verb; A1)
- `ci` - Run all quality checks in one pass
- `budget` - Enforce diff budgets on PRs
- `ratchet` - Prevent metric regressions

### Utilities
Specialized tools and integrations:
- `filter` - Filter files by glob patterns and inspect --exclude/--only aliases (`filter aliases`, `filter matches`; owned by `normalize-filter`, B6). Old top-level `aliases` kept as a hidden alias for one release.
- `context` - Show directory context (.context.md files)
- `translate` - Translate code between languages
- `guide` - Workflow guides with examples
- `generate` - Code generation from API spec
- `package` - Package management (info, list, tree, why, outdated, audit)
- `sessions` - Agent session logs (Claude Code, Codex, Gemini)

### Infrastructure
Setup, configuration, and plumbing:
- `update` - Check for and install updates
- `daemon` - Background process management
- `grammars` - Tree-sitter grammar management
- `syntax` - AST inspection (ast, query, node-types)
- `tools` - External tool orchestration (lint, test)
- `config` - Inspect and validate config files using JSON Schema
- `serve` - Server protocols (mcp, http, lsp)

## Design Principles

### 1. One namespace per concept
Bad: `grep search`, `grep find` (grep does nothing but search)
Good: `grep` (direct access to the one thing)
(A namespace earns its place once it holds >1 command: `filter` groups `filter aliases`
and `filter matches`, so it is a real namespace, not ceremony.)

### 2. Group by domain, not by verb
Bad: `list-sessions`, `list-grammars`, `list-packages`
Good: `sessions`, `grammars list`, `package list`

### 3. Subcommands for related operations
`analyze` has 45 subcommands because they're all "analysis" - one concept with variants.
This is better than 40 top-level commands that pollute the namespace.

### 4. `list` as subcommand, not flag
Consistent pattern across: `grammars list`, `daemon list`, `package list`, `tools lint list`, `tools test list`.
Not: `--list` flag (inconsistent with above).

### 5. Positional args for primary targets
`normalize view src/main.rs` not `normalize view --file src/main.rs`
`normalize sessions <id>` not `normalize sessions --id <id>`

### 6. Flags for modifiers
`--json`, `--pretty`, `--compact` - output format
`--root` - working directory
`--exclude`, `--only` - filtering

### 7. `--dry-run` on every mutating command

Every command that writes, deletes, or modifies anything must support `--dry-run` to preview what would happen without doing it. No exceptions. This applies to `edit`, `init`, `update`, `rules enable/disable`, and anything that touches files, config, or state. Read-only commands (`view`, `analyze`, `grep`) don't need it.

### 8. Filters compose

Multiple filters always AND together. There are no filter combinations that are invalid or undefined. A user who specifies `--tag debug-print --language rust --enabled` gets exactly the intersection: enabled debug-print rules for Rust. This applies uniformly across all commands that accept filters (`rules list`, `rules run`, `view`, `edit`, etc.).

Corollary: never add a special-cased filter that only works alone or only works with certain other filters. If a filter can't compose, it's a flag, not a filter.

### 9. Global flags at root level
Output format flags (`--json`, `--jq`, `--pretty`, `--compact`) are defined once at root, not duplicated per command.

## Rank output house style

All 22 `normalize rank` subcommands share one text-output house style. This is the
single source of truth; the audit at `docs/artifacts/cli-fixes-2026-06-16/rank-formatting-audit.md`
catalogues the pre-migration divergences. When migrating a subcommand or adding a new
one, conform to every rule below. The migrated **`complexity`**, **`ownership`**, and
**`coupling`** commands are the reference exemplars — copy their structure.

### Title

`# <Command Name> — <stat>, <stat>, …`

- Always `#`-prefixed (one `#`, never `##`/`###` for the title).
- Summary stats go **inline in the title**, comma-separated, never in a separate
  key-value preamble block. There is no `Root: …` / `Functions: …` block above the table.
- Diff mode prepends `Diff vs <ref>` to the command name
  (`# Complexity Diff vs HEAD~5 — …`).

Example: `# Complexity — 30 functions, avg 2.4, max 9, 0 critical, 0 high`

### Body: one `format_ranked_table`

Tabular subcommands render their body with a **single** call to
`normalize_rank::ranked::format_ranked_table(title, &entries, empty_message)`.
The entry type implements `RankEntry` (`columns()` + `values()`). This gives:
auto-width columns, `-` separators with `--` between columns, no hardcoded widths,
**no path truncation**, no row indentation. Do not hand-roll a table, do not call
`"-".repeat(n)`, do not `format!("{:<50}", …)`.

### Headers

Title-case, spelled out, no unexplained abbreviations. `Bus Factor` not `BF`,
`Confidence` not `Conf%`, `Authors` not `Auth`, `Shared Commits` not `Shared`.

### Risk tiers are a **column**, not subsections

Commands that classify rows into severity bands (`complexity`, `length`, `test-gaps`)
must NOT emit `### Critical` / `### High Risk` subsections. Instead add a `Risk` column
whose cell is the tier title. Map the command's domain thresholds onto the shared
`normalize_rank::ranked::RiskTier` (`Low`/`Moderate`/`High`/`Critical`) — see
`RiskLevel::tier()` in `complexity.rs` for the pattern. `RiskTier::title()` is the cell
text; `RiskTier::rank()` drives pretty-mode coloring via `output::tier_color`.

### Footnotes move to `--help`

No trailing footer footnotes in text output. Formula explanations
(`Confidence = shared / max(...)`), abbreviation legends, and caveats
(`low bus factor means single-author risk`) go in the subcommand's `#[cli]`
doc-comment (its `--help`), not after the table. See `RankService::coupling` /
`RankService::ownership` doc comments for where the removed footnotes landed.

### `format_pretty()` uses `nu_ansi_term`

Never raw `\x1b[...]` escapes. For row-colored tables (severity coloring) call
`output::pretty_ranked_table(title, &entries, empty_message, |e| Some(color))`, which
bolds the `#` title and colors whole data rows (alignment-safe — ANSI escapes wrap the
already-padded line and never enter the width math). Pass `|_| None` for a
bold-title-only table.

### Numbers

Bare integers. No thousands commas (`254925`, not `254,925`), no `K` suffix (`90000`,
not `90K`), no unit suffixes inside values (`13`, not `13 lines` — the unit belongs in
the column header).

### Non-tabular subcommands

`size` (tree), `duplicates` (prose groups), `duplicate-types` (numbered pairs),
`fragments` (cluster + location sub-rows), `uniqueness` (cluster list) are inherently
non-tabular. They still get the `#` title with inline stats and `nu_ansi_term`
pretty-mode, but keep their tree/prose body — they are **not** simple
`format_ranked_table` swaps. The flat-table rules (one table, spelled-out headers, no
footnotes, bare integers) still apply to any tabular sub-section they contain.

### Before / after (complexity)

```
# before                          # after
# Complexity Analysis             # Complexity — 30 functions, avg 2.4, max 9, 0 critical, 0 high
                                  
Functions: 6771 (showing 5)       Complexity  Risk      Function
Average: 4.2                      -------------------------------------------
Maximum: 90                                9  Moderate  format_ranked_table
Critical (>20): 213                        6  Moderate  compute_ranked_diff
## Complex Functions                       1  Low       Column.left
### Critical
90 file.rs:HealthReport.score
```

## Entry Points

Total: ~110 entry points (21 top-level + subcommands)

Commands with most subcommands:
- `analyze`: ~16 (health, summary, docs, security, skeleton-diff, cross-repo-health, and others — see `normalize analyze --help` for current list; many commands have been migrated to `rank`, `trend`, `syntax`, `view`, `architecture`, and `history` — the git-history cluster `hotspots`/`coupling`/`ownership`/`contributors`/`activity`/`repo-coupling`/`coupling-clusters` now lives under `history`)
- `history`: 7 (hotspots, coupling, ownership, contributors, activity, repo-coupling, coupling-clusters; owned by `normalize-git-history`)
- `syntax`: 3 (ast, query, node-types)
- `rules`: 10 (list, run, enable, disable, show, tags, add, update, remove, validate)
- `edit`: 10 (delete, replace, swap, insert, undo, redo, goto, batch, history)
- `daemon`: 7 (status, stop, start, run, add, remove, list)
- `sessions`: 6 (list, show, stats, messages, plans)
- `package`: 6 (info, list, tree, why, outdated, audit)

### `rules` subcommand surface

```
rules list     [--tag <tag>] [--language <lang>] [--enabled] [--disabled] [--type syntax|fact] [--expand]
rules run      [--tag <tag>] [--language <lang>] [--rule <id>] [--fix] [--dry-run]
rules show     <id>
rules tags     [--tag <tag>]
rules enable   <tag-or-id>   [--dry-run]
rules disable  <tag-or-id>   [--dry-run]
rules add      <url>
rules update
rules remove   <id>
rules setup
rules validate
```

`--expand` on `rules list` shows allow patterns, message, and first line of docs per rule. `rules show <id>` renders the full documentation — rationale, examples, remediation, when to disable — accessible offline.

All filters on `list` and `run` compose (see principle #8). `enable`/`disable` accept either a rule ID or a tag name — when given a tag, they apply to all rules matching that tag.

Commands with no subcommands (positional/flag-based):
- `view`, `grep`, `context`, `init`, `update`, `docs`

### `docs` — ecosystem-dispatched, not language-flagged

`normalize docs <symbol>` fetches upstream symbol documentation. Rather than a
`--language` flag, it dispatches over the project's **ecosystem** (the same
`Ecosystem` trait that backs `package`): the ecosystem owns both the
symbol-parsing convention (`crate::Sym` for Rust, `path#Sym`/`pkg.Sym` for Go,
`pkg.Sym` for Python) and the doc sources. This keeps the data model honest —
"where do docs come from" is an ecosystem question, not a syntax question — and
lets the command auto-detect from the working directory, with `-e/--ecosystem`
to disambiguate when more than one is present. Each ecosystem resolves docs
**locally first** (installed source) and falls back to the **remote package
source archive** (not a scraped docs site), so the body reflects the version in
use. Bodies are stored source-native (`doc_body` + `doc_format`); rendering to
display Markdown happens at the output layer, so `--json` consumers get the raw
text and pick their own rendering.

## Command Aliases

Users from other tools often try familiar names. These aliases are rewritten transparently in `main.rs` before server-less dispatch:

| Alias | Canonical Command | Rationale |
|-------|-------------------|-----------|
| `find` | `grep` | Common alternative for text search |
| `lint` | `rules run` | Standard linter invocation |
| `check` | `ci` | Common CI/check command name |
| `index` | `structure rebuild` | Indexing is the primary use of `structure` |
| `refactor` | `edit` | Refactoring tools use this name |

Aliases are invisible — they don't appear in `--help` output. The canonical name is always what's shown.

`search` is **not** an alias: it is the top-level semantic-search verb
(`normalize search <query>`, served by `normalize-semantic`), ranking symbols,
docs, and commits by meaning. For text/regex search use `grep` (or its `find`
alias).

> **Resolved collision (taxonomy inversion B7, 2026-07-03, EXECUTED):** the semantic-search verb
> `search` (normalize-semantic) clashed with the former `search`→`grep` alias. **Decision:** drop the
> `search`→`grep` alias and let `search` become the semantic verb. Executed at **B7**, atomically
> with mounting the verb — `search` now routes to semantic search, not grep. This is a user-facing
> behavior change. See `docs/audit-2026-07-03-command-surface-decomposition.md`.
