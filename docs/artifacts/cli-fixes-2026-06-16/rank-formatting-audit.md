# `normalize rank` Formatting Audit — 2026-06-16

Scope: all 22 subcommands of `normalize rank`. Goal: catalogue inconsistencies
concretely, with representative output snippets, so they can be fixed consistently.

---

## 1. The house style (what `format_ranked_table` does)

Most rank-pattern commands route through `format_ranked_table` in
`crates/normalize-analyze/src/ranked.rs`. Its contract:

```
# Title line — optional summary stats in the title
                              ← blank line
ColumnA  ColumnB  ColumnC    ← header, auto-width, left/right aligned per Column def
-------  -------  -------    ← dashes, same width as header, "--" between columns
row 1
row 2
...
```

The `#` prefix on the title is conventional for those reports that use it, but
`format_ranked_table` does NOT add it — callers pass the full title string. This
is the first split: some callers pass `"# Foo"`, some pass `"Foo"`.

---

## 2. Per-subcommand catalogue

### 2.1 `complexity`

**Source:** `crates/normalize/src/analyze/complexity.rs` — `ComplexityReport::format_text`

**Approach:** fully hand-rolled (no `format_ranked_table`). Uses `# title`, blank line,
key-value summary block, then `## Complex Functions` section, with `### Critical /
### High Risk / ...` subsections. Items are `<score> file:symbol` with no alignment.

**Representative output:**
```
# Complexity Analysis

Functions: 6771 (showing 5)
Average: 4.2
Maximum: 90
Critical (>20): 213
High risk (11-20): 448

## Complex Functions
### Critical
90 crates/normalize/src/health.rs:HealthReport.score_breakdown
87 crates/normalize/src/commands/sessions/messages.rs:build_messages_report
```

**Inconsistencies vs house style:**
- No table — items are bare `score name` lines with no column headers or separator.
- Risk subsections (`### Critical`, `### High Risk`) exist in no other subcommand.
- Summary stats appear as a key-value block before the list, not embedded in the title.
- Has `format_pretty()` (with color).

---

### 2.2 `length`

**Source:** `crates/normalize/src/analyze/function_length.rs` — `LengthReport::format_text`

**Approach:** hand-rolled (no `format_ranked_table`). Identical structural shape to
`complexity`. Uses `# Function Length Analysis`, key-value stats block, `## Longest
Functions`, `### Too Long / ### Long / ### Medium` subsections. Items are
`<lines> file:symbol`.

**Representative output:**
```
# Function Length Analysis

Functions: 56 (showing 5)
Average: 13.5 lines
Maximum: 172 lines
Too Long (>100): 2
Long (51-100): 1

## Longest Functions
### Too Long
172 editors/vscode/src/extension.ts:activate
```

**Inconsistencies vs house style:**
- No table. Same non-tabular style as `complexity`.
- Category subsections (`### Too Long`, `### Long`, `### Medium`) exist nowhere else.
- Stats appear before the list as a key-value block, not in the title.
- Values include "lines" units inline (`13.5 lines`, `172 lines`) — inconsistent with
  other subcommands that suppress units.
- Has `format_pretty()`.

---

### 2.3 `files`

**Source:** `crates/normalize/src/commands/analyze/files.rs` — `FileLengthReport::format_text`

**Approach:** uses `format_ranked_table`. Title has `#` prefix with inline stats.
Appends `## By Language` section with hand-rolled indented lines.

**Representative output:**
```
# Longest Files — 217283 lines across all files

Lines  Path
--------------------------------------------
17924  grammars/jinja2/src/parser.c
 7827  crates/normalize/src/rg/flags/defs.rs

## By Language
188712 lines  Rust
 18666 lines  C
```

**Inconsistencies:**
- `## By Language` section is hand-rolled (not a table), with a different column order
  (`count first, then name`) vs the `format_ranked_table` pattern (header row, dashes).
- Stats embedded in title (good pattern), but "lines" plural is spelled out in the title
  whereas `complexity`/`length` use a key-value block.

---

### 2.4 `ceremony`

**Source:** `crates/normalize/src/commands/analyze/ceremony.rs` — `CeremonyReport::format_text`

**Approach:** hybrid. Opens with a `# Ceremony Ratio — …` title and an inline summary,
then a key-value block with indented `Label : value` lines, then a `## By Language`
hand-rolled section, then delegates to `format_ranked_table` for `## Highest-Ceremony
Files`. Uses two-space indent on the key-value block.

**Representative output:**
```
# Ceremony Ratio — 0.1% (6 of 8803 callables are interface boilerplate)

  Interface impl methods :     6  (0.1%)
  Inherent/class methods :  6143  (69.8%)
  Free functions         :  2654  (30.1%)

## By Language
   10.7%  (   6/  56)  TypeScript
    0.0%  (   0/   7)  Lean

## Highest-Ceremony Files

Ratio   Impl  Total  File
...
```

**Inconsistencies:**
- Key-value block uses `:` alignment and two-space indent — unique in rank subcommands.
- `## By Language` section uses a three-column hand-rolled format (ratio, fraction,
  name) — different column order and separator style from `files`'s By Language section.
- Mixes hand-rolled and `format_ranked_table` sections within a single output.
- No `format_pretty()` despite the hand-rolled key-value section being an obvious
  candidate for color.

---

### 2.5 `hotspots`

**Source:** `crates/normalize/src/commands/analyze/hotspots.rs` — `HotspotsReport::format_text`

**Approach:** fully hand-rolled. No `#` prefix on the title. Uses `format!` for column
widths with a hardcoded `"-".repeat(N)` separator. Appends formula footnotes at the
bottom. Multi-repo mode wraps entries in `=== repo_name ===` fencing.

**Representative output:**
```
Git Hotspots (high churn)

File                                                Commits    Churn   Cplx    Score
--------------------------------------------------------------------------------------
crates/normalize/src/service/analyze.rs                 100     6378     10    27628
...

Score = commits × √churn × log₂(1 + max_complexity)
High scores indicate complex, bug-prone files that change often.
```

**Inconsistencies:**
- Title has NO `#` prefix — the only non-trivial list command without one.
- Column widths are hardcoded (`{:<50}`, `{:>8}`, etc.) rather than auto-width from
  `format_ranked_table`. Long paths are truncated to 48 chars via a `truncate_path`
  call, while `format_ranked_table` never truncates.
- Footer formula text appears after the table; no other subcommand adds footnotes.
- Multi-repo uses `=== name ===` fencing (shared with `coupling`, `ownership`).
- No `format_pretty()` despite `resolve_format` being called.
- Separator line is `"-".repeat(86)` vs `format_ranked_table`'s auto-width `"--"`
  between columns.

---

### 2.6 `coupling`

**Source:** `crates/normalize/src/commands/analyze/coupling.rs` — `CouplingReport::format_text`

**Approach:** delegates to `format_ranked_table` (via `format_coupling_data`), then
appends a two-line footnote. Multi-repo uses `=== name ===` fencing.

**Representative output:**
```
# Temporal Coupling (files that change together)

File A                                        File B                                   Shared  Conf%
----------------------------------------------------------------------------------------------------
...

Confidence = shared commits / max(commits_a, commits_b)
High coupling may indicate hidden dependencies or shotgun surgery.
```

**Inconsistencies:**
- Footer footnote (same pattern as `hotspots`) — no other `format_ranked_table` command
  adds trailing text.
- Two-column path display (File A, File B) is unique to this subcommand; others have a
  single path column.
- `Conf%` column header abbreviates; `ownership` uses `BF` (also abbreviated).
- Multi-repo uses `=== name ===` fencing.

---

### 2.7 `ownership`

**Source:** `crates/normalize/src/commands/analyze/ownership.rs` — `OwnershipReport::format_text`

**Approach:** delegates to `format_ranked_table` (via `format_ownership_data`), then
appends a one-line footnote. Multi-repo uses `=== name ===` fencing.

**Representative output:**
```
# File Ownership (git blame)

File                                   Lines  Auth  BF  Top Author
----------------------------------------------------------------------
grammars/jinja2/src/parser.c           17924     1   1  pterror (100%)
...

BF = Bus Factor (authors needed for >50% ownership)
Low bus factor (1) means single-author risk.
```

**Inconsistencies:**
- `BF` abbreviation is unexpanded in the table header; the footnote defines it.
  No other subcommand uses an unexpanded abbreviation in a column header.
- Footer footnote (shared with `coupling`, `hotspots`).
- Multi-repo uses `=== name ===` fencing.
- No `format_pretty()`.

---

### 2.8 `size`

**Source:** `crates/normalize/src/commands/analyze/size.rs` — `SizeReport::format_text`

**Approach:** fully hand-rolled tree renderer. Uses `# Code Size:` title with a trailing
`\n` (extra blank line baked into the title string). Uses `└──`/`├──`/`│` box-drawing
characters and indented recursion.

**Representative output:**
```
# Code Size: normalize (217283 lines)

└── 192289   88.5%  crates
    ├── 85618    39.4%  normalize
    │   ├── 85346    39.3%  src
```

**Inconsistencies:**
- Tree layout is entirely unique — not a ranked list at all. This is arguably correct
  for this command (it's hierarchical, not a flat rank), but it stands apart from all
  other subcommands.
- Column order is `lines | pct | name` — different from flat-list commands that put
  `name` (path/module) first or last.
- `{:<6}` for lines is left-aligned with a fixed 6-char field, unlike
  `format_ranked_table`'s auto-width right-alignment for numeric columns.
- No `format_pretty()`.
- Title has a trailing `\n` embedded in the push string, producing an extra blank line
  before the first tree node. Other commands let `format_ranked_table` produce exactly
  one blank separator.

---

### 2.9 `density`

**Source:** `crates/normalize/src/commands/analyze/density.rs` — `DensityReport::format_text`

**Approach:** hand-rolled key-value preamble (no `# `— wait, it does use `#`) followed
by two `format_ranked_table` sections: Modules and Most Repetitive Files. The preamble
is a single `push_str` with embedded `\n`.

**Representative output:**
```
# Code Density Analysis

Root:              normalize
Files analyzed:    2072
Compression ratio: 0.44  (lower = more repetitive)
Token uniqueness:  0.51  (lower = more repetitive)

## Modules (most repetitive first)

Module                                     Files  Compress  Unique  Density  Lines
...

## Most Repetitive Files

Density  File                                     Lines
...
```

**Inconsistencies:**
- Key-value preamble uses `label: value` (colon-separated, aligned with spaces) —
  same pattern as `uniqueness`, `call-complexity`, `module-health`. But the
  column-width alignment is different (density uses `{:<18}` padding, others use
  different widths).
- Compression ratio uses `{:.2}` (2 decimal places) in the preamble but the table
  shows `{:.3}` (3 decimal places). Inconsistent precision.
- Two sub-tables in one output is shared only with `density` and `layering`
  (`layering` has modules + layer summary).
- Has `format_pretty()`.

---

### 2.10 `uniqueness`

**Source:** `crates/normalize/src/commands/analyze/uniqueness.rs` — `UniquenessReport::format_text`

**Approach:** hand-rolled key-value preamble (colon-separated, aligned), then
`format_ranked_table` for modules, then a hand-rolled numbered list for top clusters.

**Representative output:**
```
# Function Uniqueness Analysis

Root:                 normalize
Files analyzed:       470
Functions analyzed:   6778
Unique functions:     6240  (92.1%)
Clustered functions:  538  (7.9%)
Similarity threshold: 80%

## Modules (most clustered first)

Module                         Fns  Unique  Clustered  Ratio
...

## Top Structural Clusters

1. 59 functions  727 lines  representative: ...
2. 57 functions  955 lines  ...
```

**Inconsistencies:**
- Preamble alignment uses `{:<22}` (wide label column) — different width from
  `density` (`{:<18}`), `call-complexity` (`{:<20}`), `module-health` (no alignment).
- Cluster list uses a numbered prose format (`N. X functions Y lines representative:`)
  that is unique — no table structure for the clusters section.
- `format_pretty()` exists.

---

### 2.11 `imports`

**Source:** `crates/normalize/src/commands/analyze/imports.rs` — `ImportCentralityReport::format_text`

**Approach:** single call to `format_ranked_table` with the `#` title containing inline
stats. Simplest possible implementation.

**Representative output:**
```
# Import Centrality (all modules) — 247 modules, 1891 imports

Module  Fan-in  Imported names
...
```

**Inconsistencies:** minimal — this is close to the ideal pattern. No preamble, no
footer, clean table. The only oddity is that `fan-in` is a hyphenated column header
while other commands use plain words (`Lines`, `Ratio`, `Score`).

---

### 2.12 `surface`

**Source:** `crates/normalize/src/commands/analyze/surface.rs` — `SurfaceReport::format_text`

**Approach:** single call to `format_ranked_table`. Inline stats in title.
Has `format_pretty()` with raw ANSI escape codes (not `nu_ansi_term`).

**Representative output:**
```
# Surface Area — 247 files, avg public ratio 74%, 56 fully public, max constraint 0

Module  Total  Public  Private  Ratio  Fan-in  Constraint
...
```

**Inconsistencies:**
- `format_pretty()` uses raw `\x1b[...]` escape sequences instead of `nu_ansi_term`
  (used by `density`, `uniqueness`, `module-health`, `length`, `budget`). Two different
  color APIs in the same binary.
- Wide stat-rich title; compared to `imports` the title is longer but same pattern.

---

### 2.13 `depth-map`

**Source:** `crates/normalize/src/commands/analyze/depth_map.rs` — `DepthMapReport::format_text`

**Approach:** single call to `format_ranked_table`. Inline stats in title.
Has `format_pretty()` with raw `\x1b[...]` escape sequences.

**Representative output:**
```
# Depth Map — 247 modules, max depth 5, avg 1.4, 41 entry points

Module  Depth  Fan-in  Fan-out  Downstream  Ripple Score
...
```

**Inconsistencies:**
- Raw ANSI escape codes in `format_pretty()` (shared with `surface`, `layering`).
- Otherwise close to the ideal pattern.

---

### 2.14 `layering`

**Source:** `crates/normalize/src/commands/analyze/layering.rs` — `LayeringReport::format_text`

**Approach:** two `format_ranked_table` calls: modules table + layer summary table.
Has `format_pretty()` with raw `\x1b[...]`.

**Representative output:**
```
# Layering — 247 files, 5 layers, avg compliance 95%, worst: . (95%)

Module  Layer  Cross  Down  Up  Self  Compliance
...

## Layer Summary

Layer  Modules  Avg Depth  Compliance  Upward
...
```

**Inconsistencies:**
- Raw ANSI in `format_pretty()`.
- Two-table layout (shared with `density`).
- Column headers: `Down`, `Up`, `Self` are heavily abbreviated.

---

### 2.15 `module-health`

**Source:** `crates/normalize/src/commands/analyze/module_health.rs` — `ModuleHealthReport::format_text`

**Approach:** fully hand-rolled. Key-value preamble (no colon alignment, just `format!`),
then a custom-built table with a manual header and separator, NOT using `format_ranked_table`.
Does NOT implement `RankEntry`. Uses column widths computed per-run.

**Representative output:**
```
# Module Health

Root:            normalize
Modules scored:  47

  module                          score   test   uniq  density    cerem    logic   lines
  ------------------------------  -----  -----  -----  -------  -------  -------  ------
  crates/normalize-package-index    64%     0%    98%    0.308       0%     100%   18793
```

**Inconsistencies:**
- Does NOT use `format_ranked_table` even though it has a tabular layout. Builds its own
  header and separator manually. No `RankEntry` impl.
- Two-space indent on header and rows (all other `format_ranked_table` outputs have no
  indent).
- Column headers are lowercase (`module`, `score`, `test`) — all other tables use
  title-case or ALL-CAPS column names.
- Separator uses `-` characters that mirror the column widths manually (`"-".repeat(w)`),
  same semantic as `format_ranked_table` but not sharing code.
- Metric columns mix `%` suffix on some (`score`, `test`, `uniq`, `cerem`, `logic`) and
  decimal for `density`. This is the only command to present six metrics side-by-side in
  one row.
- Has `format_pretty()` using `nu_ansi_term`.

---

### 2.16 `call-complexity`

**Source:** `crates/normalize/src/commands/analyze/call_complexity.rs` — `CallComplexityReport::format_text`

**Approach:** fully hand-rolled. Key-value preamble, then two hand-rolled sections:
`## Top Amplified` and `## Highest Reachable CC`. Custom column headers and separator
in each section. Does NOT use `format_ranked_table`. Third section `## By Module` is
also hand-rolled (but uses similar preamble style).

**Representative output:**
```
# Call-Complexity Analysis

Root:               normalize
Index available:    false
Functions analyzed: 11017
Unresolved callees: 72.0%

## Top Amplified (dispatcher → complex territory)

  amplif    local     reach     reach#    symbol
    1051.0x        1      1051       143  crates/...
```

**Inconsistencies:**
- Hand-rolled table with two-space indent and `{:<8}` fixed-width columns. Neither
  auto-width nor using `format_ranked_table`.
- `amplif`, `reach#` are truncated column headers (inconsistent with other commands).
- `{:>8.1}x` — the `x` suffix on amplification values is embedded in the value string,
  not the column header.
- Three sections, each with its own header and column format.
- Does NOT implement `RankEntry`; no `format_pretty()`.

---

### 2.17 `duplicates`

**Source:** `crates/normalize/src/commands/analyze/duplicates_views.rs` — `DuplicatesReport::format_text`

**Approach:** fully hand-rolled. Key-value stats block (no alignment), then groups
rendered as prose paragraphs. Title has NO `#` prefix. Variable structure depending on
mode (exact/similar/clusters).

**Representative output:**
```
Duplicate Function Detection

Files scanned:      684
Functions hashed:   6769
Duplicate groups:   0
Duplicated lines:   ~0
Suppressed: 139 same-name groups (...)

No duplicate functions detected.
```

**Inconsistencies:**
- No `#` prefix on title (shared with `hotspots` and `contributors`).
- Key-value stats use mixed alignment: some use `{:<20}` padding, some use spaces
  inline — inconsistent within the same file.
- When groups exist, items are rendered as prose paragraphs with `--- file:line:col ---`
  separators, not as table rows.
- No `format_pretty()`.
- This is the most complex formatter — 200+ lines of conditional logic.

---

### 2.18 `duplicate-types`

**Source:** `crates/normalize/src/commands/analyze/duplicates.rs` — `DuplicateTypesReport::format_text`

**Approach:** hand-rolled. Key-value stats (no alignment), then a numbered-list format
with three-space indent.

**Representative output:**
```
Duplicate Type Detection

Files scanned: 2501
Types analyzed: 908
Duplicate pairs: 79
Min overlap: 70%

Potential Duplicates (sorted by overlap):

1. 100% overlap (5 common fields):
   TypeA (file:37) - 5 fields
   TypeB (file:163) - 5 fields
   Common: field1, field2, field3
```

**Inconsistencies:**
- No `#` prefix on title (shared with `hotspots`, `duplicates`, `contributors`).
- Numbered-list format is unique to this and `uniqueness`'s cluster section.
- Key-value stats not aligned (e.g., `Files scanned: 2501` vs `Types analyzed: 908`
  — different label lengths, no padding).
- Hardcoded limit of 20 in the formatter (`iter().take(20)`) rather than the service
  layer passing `limit`.
- Appends a `To suppress:` command hint at the bottom — only this command does this.
- No `format_pretty()`.

---

### 2.19 `fragments`

**Source:** `crates/normalize/src/commands/analyze/fragments.rs` — `FragmentsReport::format_text`

**Approach:** hand-rolled. Title uses `#` prefix with inline stats. Then a custom
four-column table with hardcoded widths, followed by per-cluster rows with indented
location lines.

**Representative output:**
```
# Fragment Analysis (242219 fragments → 3 clusters, 62223 unclustered, min_nodes=10, inline_depth=0)

Hash                Freq    TotalLn      AvgLn  Kind / Label
------------------------------------------------------------------------
cef9af040e83       2994       4525        1.5  field_expression [...]
  benches/benches/cli_commands.rs:7-7 (normalize_binary)
  ... and 2991 more
```

**Inconsistencies:**
- Title has inline params (`min_nodes=10`, `inline_depth=0`) in `key=value` format —
  unique to this command.
- Hand-rolled table uses `{:<18}` for Hash, `{:>5}` for Freq, etc. — fixed widths,
  not auto-width.
- Column header `TotalLn`/`AvgLn` use abbreviated CamelCase; no separator between
  the header row and the separator line (`"-".repeat(72)` is hardcoded).
- Location sub-rows are two-space-indented plain text under each cluster row.
- Has `format_pretty()` (not observed in output check — confirm separately).

---

### 2.20 `test-ratio`

**Source:** `crates/normalize/src/commands/analyze/test_ratio.rs` — `TestRatioReport::format_text`

**Approach:** single call to `format_ranked_table` with the `#` title containing inline
stats. Closest to the ideal pattern.

**Representative output:**
```
# Test/Impl Ratio: normalize — 11.4% (329747 impl, 42466 test)

Module           Impl  Test  Ratio
----------------------------------
docs            39645     0   0.0%
grammars/jinja2 26975     0   0.0%
```

**Inconsistencies:** minimal — this is close to ideal. The title includes `:
<root-path>` which is slightly different from the `— stat stat` pattern used by
`files`, `coupling`, `imports`.

---

### 2.21 `budget`

**Source:** `crates/normalize/src/commands/analyze/budget.rs` — `LineBudgetReport::format_text`

**Approach:** two `format_ranked_table` calls: categories table + by-module table.
Has `format_pretty()` with `nu_ansi_term` that replaces the categories table with a bar
chart, and the by-module table with a hand-rolled prose format.

**Representative output:**
```
# Line Budget: normalize (391K lines)

Category          Lines    Pct
------------------------------
business logic  254,925  65.3%
documentation    45,205  11.6%
...

## By Module

Module                      Lines  Logic  Test  Other
-----------------------------------------------------
crates/normalize              90K    92%    8%     0%
```

**Inconsistencies:**
- Lines in categories table are formatted with commas (`254,925`) via a `format_num`
  helper; no other subcommand applies comma formatting to numbers.
- In the by-module table, lines are in `K` (kiloline) units (`90K`) rounded, while
  `files` shows exact line counts. Two different precision conventions for line counts.
- `format_pretty()` renders a bar chart for categories — a different _kind_ of
  visualization, not just color on the same structure. The pretty-mode by-module
  section also collapses to inline prose (`90K  (92% logic, 8% test, 0% other)`),
  removing the columnar table structure entirely.
- Two-table layout (categories + modules).

---

### 2.22 `test-gaps`

**Source:** `crates/normalize/src/analyze/test_gaps.rs` — `TestGapsReport::format_text`

**Approach:** hand-rolled. No `#` prefix on opening line. Key-value preamble (no
alignment). Then a hand-rolled table using `─` (em-dash) as separator character.

**Representative output:**
```
Test Gaps: 1453 of 1574 public functions have no direct test

   Risk  Function                              File                      Complexity  Callers  LOC
 ──────  ────────────────────────────────────  ────────────────────────  ──────────  ───────  ───
  451.6  SessionAnalysisReport.format_text     crates/normalize-sess...          68        2  421
```

**Inconsistencies:**
- No `#` prefix (shared with `hotspots`, `duplicates`, `duplicate-types`,
  `contributors`).
- Uses `─` (U+2500 BOX DRAWINGS LIGHT HORIZONTAL) as separator instead of `-`
  (U+002D HYPHEN-MINUS) used by all other commands. The only command to do this.
- Table has two-space indent on all rows.
- Fixed-width columns (`{:<36}`, `{:<24}`, etc.) with paths truncated to 24 chars.
- Column `LOC` is an abbreviation with no footnote.
- Paths are truncated mid-string to fit fixed columns rather than letting `format_ranked_table`
  auto-size.
- No `format_pretty()`.

---

### 2.23 `contributors`

**Source:** `crates/normalize/src/commands/analyze/contributors.rs` — `ContributorsReport::format_text`

**Approach:** three hand-rolled sections separated by blank lines: Author Summary,
Repo Summary, Author Overlap. Each section has its own hardcoded column widths. No
`#` prefix anywhere (not even on section headers).

**Representative output:**
```
Author Summary

Author                         Repos  Commits Top Repo (%)
----------------------------------------------------------------------
pterror                           30    13050 normalize (26%)

Repo Summary

Repo                      Authors  Commits  BF Top Author (%)
---------------------------------------------------------------------------
```

**Inconsistencies:**
- No `#` prefix on any title or section header — uses plain undecorated strings.
- Three completely separate tables, each hand-rolled with different column widths and
  separator lengths.
- Separator length differs between sections (`"-".repeat(70)` vs `"-".repeat(75)`
  vs `"-".repeat(65)`).
- `BF` abbreviation not defined anywhere in output (unlike `ownership` which has a
  footnote).
- No `format_pretty()`.

---

## 3. Inconsistency catalogue

| Dimension | Consistent practice | Exceptions |
|-----------|---------------------|-----------|
| **`#` prefix on title** | Used by 17/22 subcommands | `hotspots`, `duplicates`, `duplicate-types`, `test-gaps`, `contributors` (5 subcommands) |
| **Tabular rendering** | `format_ranked_table` (auto-width, header, `--` between columns) | `complexity`, `length`, `hotspots`, `module-health`, `call-complexity`, `duplicates`, `duplicate-types`, `test-gaps`, `contributors`, `fragments` (10 subcommands roll their own) |
| **Stats in title vs. preamble block** | Title: `files`, `coupling`, `ownership`, `imports`, `surface`, `depth-map`, `layering`, `test-ratio`, `budget`, `size` | Preamble block: `complexity`, `length`, `ceremony`, `density`, `uniqueness`, `call-complexity`, `module-health` |
| **Footer/footnote text** | Not present in most | `hotspots`, `coupling`, `ownership`, `duplicate-types` |
| **Separator character** | `-` (ASCII hyphen) | `test-gaps` uses `─` (U+2500) |
| **color/pretty API** | `nu_ansi_term` in `format_pretty()` | `surface`, `depth-map`, `layering` use raw `\x1b[...]` ANSI codes |
| **Table indentation** | No indent | `module-health`, `call-complexity`, `test-gaps` add two-space indent |
| **Number formatting** | Plain integers | `budget` uses comma-formatted numbers (`254,925`); `size` uses `K` suffix |
| **Multi-repo fencing** | `=== name ===` | `hotspots`, `coupling`, `ownership` (consistent with each other) |
| **Column header case** | Title-case (`Lines`, `Module`, `Ratio`) | `module-health` uses lowercase (`module`, `score`); `call-complexity` uses `amplif`, `reach#` |
| **Path truncation** | None (auto-width) | `hotspots` truncates to 48; `test-gaps` truncates to 24; `call-complexity` to ~40 |
| **Units in values** | Bare numbers | `length` adds `" lines"` suffix; `budget` uses `K` suffix for modules |
| **Risk/category subsections** | Not used | `complexity` and `length` both use `### Critical`/`### High Risk` etc. |

---

## 4. Common report struct fields

The rank subcommands' report structs share certain field patterns:

- **`diff_ref: Option<String>`** — present in all diffable subcommands (14/22):
  `complexity`, `length`, `files`, `ceremony`, `coupling`, `ownership`, `test-ratio`,
  `budget`, `density`, `uniqueness`, `imports`, `surface`, `depth-map`, `layering`.
  NOT present in: `hotspots` (has `recency_weighted` instead), `call-complexity`,
  `duplicates`, `duplicate-types`, `fragments`, `test-gaps`, `module-health`,
  `contributors`, `size`.

- **`root: String`** — present in `density`, `uniqueness`, `call-complexity`,
  `module-health`, `test-ratio`, `budget`. The six subcommands that have a key-value
  preamble block all include `root`.

- **`delta: Option<f64>` on entry structs** — present on all entry structs that
  implement `DiffableRankEntry`.

- **No shared base struct** — there is no common `RankReport<T>` wrapper. Each report
  struct is independent.

---

## 5. House style recommendation

A consistent house style for all `rank` subcommands should be:

### Title line
`# <Command Name> [— <stat>, <stat>, ...]`

The `#` prefix is already used by 17/22 commands. The 5 outliers should be updated.
Stats that contextualize the table (total files, overall ratio) belong in the title,
not in a separate preamble block. Commands that currently use preamble blocks
(`complexity`, `length`, `ceremony`, `density`, `uniqueness`, `call-complexity`,
`module-health`) should condense the key stats into the title and drop the block — or
at minimum standardize on a shared preamble format.

### Table body
All tabular rank outputs should use `format_ranked_table`. The 10 commands that
hand-roll their own table should migrate. The payoff: auto-width columns, consistent
`--` separators, no hardcoded widths, no path truncation.

For commands whose output is inherently non-tabular (`size`, `duplicates`,
`duplicate-types`, `fragments`), the current approach may be appropriate — but they
should still adopt the `# Title` prefix.

### No footer footnotes
The `coupling`, `ownership`, `hotspots` footnotes explaining formulas/abbreviations
should be removed or moved into `--help`. Abbreviations (`BF`, `Conf%`, `amplif`) in
column headers should be spelled out.

### No category subsections
`complexity` and `length` use `### Critical` / `### Too Long` subsections inside their
output. This is the only place `###` appears in rank output. The information should
either be a column in the table (e.g., a `Risk` column) or be dropped.

### `format_pretty()` consistency
Of the 22 subcommands, 10 have `format_pretty()`. Of those, 3 (`surface`, `depth-map`,
`layering`) use raw `\x1b[...]` escape codes. All `format_pretty()` implementations
should use `nu_ansi_term`.

### Number formatting
Bare integers for all counts. No comma formatting (`budget`), no `K` suffix (`budget`
module table), no `" lines"` suffix (`length`).

### Multi-repo fencing
The `=== name ===` pattern used by `hotspots`, `coupling`, `ownership` is an
acceptable consistent sub-convention for multi-repo outputs. It should be extracted
into a shared helper if other commands add multi-repo support.

---

## 6. Priority ranking of fixes

1. **Add `#` prefix to `hotspots`, `duplicates`, `duplicate-types`, `test-gaps`,
   `contributors`** — cosmetic but high-visibility.
2. **Migrate `test-gaps` separator from `─` to `-`** — only one character but causes
   a jarring visual inconsistency.
3. **Migrate `module-health` and `call-complexity` to `format_ranked_table`** — they
   are tabular and have the right data shape.
4. **Switch `surface`, `depth-map`, `layering` `format_pretty()` from raw ANSI to
   `nu_ansi_term`** — correctness/maintainability issue.
5. **Spell out column abbreviations** (`BF`, `Conf%`, `amplif`, `reach#`, `Auth`, `LOC`,
   `TotalLn`, `AvgLn`) — readability.
6. **Condense preamble-block commands into title-inline stats** — the six preamble-block
   commands (`complexity`, `length`, `density`, `uniqueness`, `call-complexity`,
   `module-health`) are the largest divergence from the table-centric house style.
7. **Drop footer footnotes** from `coupling`, `ownership`, `hotspots` — the information
   belongs in `--help`.
8. **Standardize number format** — remove commas, `K` suffix, and `" lines"` units
   from values.
