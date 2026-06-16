# rank-consistency-verification.md

Independent post-migration verification of all 22 `normalize rank` subcommands against
the house style spec in `docs/cli-design.md` § "Rank output house style".
Performed 2026-06-16 against `./target/debug/normalize` (build confirmed up to date).

---

## Conformance Table

Legend: ✓ = pass, ✗ = fail, ~ = partial/borderline, N/A = not applicable (non-tabular)

| # | Command | `#` title w/ inline stats | Ranked table or prose body | Headers spelled out | No body footnotes | Bare integers | No raw `\x1b` in source |
|---|---------|--------------------------|---------------------------|---------------------|-------------------|---------------|-------------------------|
| 1 | `complexity` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 2 | `ceremony` | ✓ | ✓ | ~ | ✓ | ✓ | ✓ |
| 3 | `length` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 4 | `uniqueness` | ✓ | N/A (prose) | ~ | ✓ | ✓ | ✓ |
| 5 | `call-complexity` | ✓ | ✓ | ~ | ✓ | ✓ | ✓ |
| 6 | `duplicates` | ✓ | N/A (prose) | ✓ | ~ | ✓ | ✓ |
| 7 | `duplicate-types` | ✓ | N/A (prose) | ✓ | ✓ | ✓ | ✓ |
| 8 | `fragments` | ✓ | N/A (prose) | ✓ | ✓ | ✓ | ✓ |
| 9 | `size` | ✓ | N/A (tree) | N/A | ✓ | ✓ | ✓ |
| 10 | `density` | ✓ | ✓ | ~ | ✓ | ✓ | ✓ |
| 11 | `imports` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 12 | `surface` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 13 | `depth-map` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 14 | `layering` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 15 | `module-health` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 16 | `files` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 17 | `hotspots` | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 18 | `coupling` | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 19 | `ownership` | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 20 | `contributors` | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 21 | `test-ratio` | ~ | ✓ | ~ | ✓ | ✓ | ✓ |
| 22 | `test-gaps` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 23 | `budget` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

**Fully conforming (all rules pass): 14 of 22.**
**Deviations found: 8 commands (some overlap between rules).**

---

## Deviations — Detailed

### DEV-1: `hotspots` — no inline stats in title

**Rule violated:** Title must be `# <Command Name> — <stat>, <stat>, …` with inline
summary stats. There is no separate preamble block.

**Actual output:**
```
# Git Hotspots (high churn)
```

**Expected form (illustrative):**
```
# Git Hotspots — 20 files, top score 27628, max churn 6378
```

**Source:** `crates/normalize/src/commands/analyze/hotspots.rs`, `hotspots_title()` at L100-106.
The function returns a bare descriptive string with no `—` separator and no stats.
The stats (file count, max score, max churn) exist in the `HotspotsReport` struct but are
not threaded into the title.

---

### DEV-2: `coupling` — no inline stats in title

**Rule violated:** Title must carry inline summary stats.

**Actual output:**
```
# Temporal Coupling (files that change together)
```

**Expected form (illustrative):**
```
# Temporal Coupling — 20 pairs, max confidence 58%
```

**Source:** `crates/normalize/src/commands/analyze/coupling.rs`, `format_coupling_data()` at
L95-103. Hardcoded title string with no stat interpolation.

---

### DEV-3: `ownership` — no inline stats in title

**Rule violated:** Title must carry inline summary stats.

**Actual output:**
```
# File Ownership (git blame)
```

**Expected form (illustrative):**
```
# File Ownership — 38 files, avg bus factor 1.0, 38 single-author
```

**Source:** `crates/normalize/src/commands/analyze/ownership.rs`, `format_ownership_data()` at
L95-103. Hardcoded title string; stats are in `OwnershipReport` fields but unused in title.

---

### DEV-4: `contributors` — no command-level title at all; no inline stats

**Rule violated:** Title must be `# <Command Name> — <stat>, <stat>, …`. The contributors
command emits two or three separate sub-table titles (`# Author Summary`, `# Repo Summary`,
`# Author Overlap`) with no outer command-level title and no aggregate inline stats.

**Actual first line of output:**
```
# Author Summary
```

There is no `# Contributors — …` title preceding the tables.

**Source:** `crates/normalize/src/commands/analyze/contributors.rs`, `format_text()` at L111-131.
Each table is titled independently; there is no top-level title with inline stats.

---

### DEV-5: `ceremony` — abbreviated column header `Impl` in per-file table

**Rule violated:** Headers must be "title-case, spelled out, no unexplained abbreviations."

**Actual per-file table header:**
```
Ratio  Impl  Total  File
```

`Impl` is an abbreviation for `Implementation` (or "Interface Implementation"). The per-language
breakdown correctly uses `Interface Impl` (still abbreviated but at least scoped). The per-file
column should be `Implementations` or `Impl Count`.

**Source:** `crates/normalize/src/commands/analyze/ceremony.rs`, `FileCeremony::columns()` at
L74-81, line 77: `Column::right("Impl")`.

Note: `LangCeremonyEntry::columns()` uses `"Interface Impl"` (still abbreviated but explanatory).
The asymmetry between the two tables in the same command is also a minor inconsistency.

---

### DEV-6: `density` — abbreviated column header `Compress`

**Rule violated:** Headers must be spelled out; no unexplained abbreviations.

**Actual module table header:**
```
Module  Files  Compress  Unique  Density  Lines
```

`Compress` is an abbreviation for `Compression`. The column represents the compression ratio
(0.0–1.0), which is not obvious from `Compress` alone.

**Source:** `crates/normalize/src/commands/analyze/density.rs`, `ModuleDensity::columns()` at
L60-69, line 64: `Column::right("Compress")`.

---

### DEV-7: `call-complexity` — abbreviated column headers `Local CC`, `Reachable CC`

**Rule violated:** No unexplained abbreviations. `CC` = cyclomatic complexity, which is
not spelled out anywhere in the text output itself (only in `--help`).

**Actual headers (two tables):**
```
Amplification  Local CC  Reachable CC  Reachable Count  Symbol
Reachable CC   Local CC  Reachable Count  Symbol
Functions  Avg Amplification  Max Reachable CC  Local CC  Module
```

`CC` is an unexplained abbreviation in the text output. Spec examples use `Bus Factor` not `BF`,
`Confidence` not `Conf%`. By the same logic, `Local CC` → `Local Complexity` and
`Reachable CC` → `Reachable Complexity`.

**Source:** `crates/normalize/src/commands/analyze/call_complexity.rs`, three `columns()`
implementations at L32-40, L58-65, L89-97.

---

### DEV-8: `uniqueness` — abbreviated column header `Fns`

**Rule violated:** No unexplained abbreviations.

**Actual Modules table header:**
```
Module   Fns  Unique  Clustered  Ratio
```

`Fns` is an abbreviation for `Functions`.

**Source:** `crates/normalize/src/commands/analyze/uniqueness.rs`, `ModuleUniqueness::columns()`
at L31-39, line 34: `Column::right("Fns")`.

---

### DEV-9 (borderline): `test-ratio` — abbreviation `Impl` in title and column

**Rule violated:** `Impl` is an abbreviation in both the command title and the table column.

**Actual title:**
```
# Test/Impl Ratio — 7.2% (78794 impl, 6081 test)
```

**Actual column:**
```
Module   Impl  Test  Ratio
```

`Impl` appears three times (title word, paren stat, column header). The abbreviated form is
arguably idiomatic ("impl" is Rust jargon for "implementation") but the spec's intent
("spelled out") would prefer `Implementation` or `Production`.

**Source:** `crates/normalize/src/commands/analyze/test_ratio.rs`:
- Title string at L90-98: `"# Test/Impl Ratio"`, inline stat `{} impl`
- Column at L35: `Column::right("Impl")`

---

### DEV-10 (borderline): `duplicates` — suppression lines contain flag references

**Rule violated:** "No trailing footer footnotes in text output." The spec says formula
explanations and caveats go in `--help`, not in output.

**Actual output when suppressions exist:**
```
Duplicated lines: ~0
Suppressed: 61 same-name groups (likely trait impls; use --include-trait-imps to show)
```

The `use --include-trait-imps to show` clause is a user-guidance reference to a CLI flag
embedded in the body. The spec says this belongs in `--help`. However, these lines appear
before the main body (not as a trailing footer), so they are a preamble rather than a
footnote — this is a grey area. The `Duplicated lines: ~0` line is a summary stat that
arguably belongs in the title. When there are no groups, both lines appear before the
empty-message, making them effectively a preamble block rather than a trailing footnote.

**Source:** `crates/normalize/src/commands/analyze/duplicates_views.rs`, `format_text()` at
L264-325.

This is listed as borderline because duplicates is non-tabular (explicitly exempted from the
`format_ranked_table` requirement) and the suppression info is contextually necessary. The
`--include-trait-impls` hint in output does push it toward footnote territory.

---

## Raw ANSI (`\x1b`) Audit

Searched all `crates/normalize/src/**/*.rs` files. Raw `\x1b` escapes found in:

- `commands/analyze/cross_repo_health.rs` — `analyze cross-repo-health` (not a `rank` subcommand)
- `commands/analyze/skeleton_diff.rs` — `analyze skeleton-diff` (not a `rank` subcommand)
- `commands/analyze/trend.rs` — `analyze trend` (not a `rank` subcommand)
- Various non-rank commands (`find_references`, `sessions/*`, `view/report.rs`, `service/edit.rs`, etc.)

**No raw `\x1b` escapes in any `rank` subcommand source.** All rank commands use `nu_ansi_term`
(confirmed: `pretty_ranked_table` / `output::tier_color` pattern, no literal escape bytes).

---

## Number Formatting Audit

Searched for `{:,}`, `format_thousands`, comma-separator patterns, and `K`/unit suffixes
in rank subcommand sources. **None found.** All integer values in rank output use bare
`integer.to_string()` or bare `format!("{}", n)` — no thousands commas, no `K` suffixes,
no unit suffixes inside value cells.

---

## `cargo test -q` Result

```
test result: ok. 409 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.53s
test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.89s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**All tests pass. Zero failures.** The one ignored test is the known-flaky daemon test
`config_edit_triggers_reload_event` (fs-watch timing, unrelated to formatting).

---

## Verdict

**The migration achieved significant progress but is not complete.** 14 of 22 commands
fully conform. The 8 remaining deviations break down into two categories:

**Hard failures (title rule — no inline stats):** `hotspots`, `coupling`, `ownership`,
`contributors`. These four commands have descriptive titles with no `—` separator and no
summary statistics inline. This is the most visible spec violation: looking at the output
headers, these four are immediately identifiable as pre-migration style.

**Header abbreviation failures:** `ceremony` (`Impl`), `density` (`Compress`),
`call-complexity` (`Local CC`, `Reachable CC`, `Max Reachable CC`), `uniqueness` (`Fns`),
`test-ratio` (`Impl`). Five commands use abbreviated column headers that the spec explicitly
disallows. `CC` in particular is unexplained in output — it only appears spelled out in
`--help`.

**No raw ANSI violations, no thousands-comma violations, no K-suffix violations anywhere
in rank subcommand sources.** The formatting infrastructure (nu_ansi_term, format_ranked_table,
bare integers) is uniformly applied. The remaining gaps are specifically in title construction
and column naming — mechanical but still present.

The group does **not** present a consistent house style. The four no-stats titles are the
most jarring divergence; the abbreviation issues are secondary.
