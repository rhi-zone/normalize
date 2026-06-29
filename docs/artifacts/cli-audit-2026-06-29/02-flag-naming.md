# CLI Flag-Naming Audit — 2026-06-29

Auditor: Claude Code (claude-sonnet-4-6)
Scope: All `normalize` subcommands enumerated via `--help` + `#[param(name=...)]` source grep.
Binary: `/home/me/git/rhizone/normalize/target/debug/normalize` (built against server-less 0.6.0 path override)

---

## 1. server-less 0.6 Rename Risk

### Background

server-less 0.6.0 begins honoring `#[param(name = "...")]` in CLI codegen.
Prior to 0.6.0 the attribute was silently ignored and the flag name was derived from the
kebab-cased Rust field/param identifier.

Any param where `name = "..."` differs from the kebab-case of its Rust identifier has a
**silently changed CLI flag name** when upgrading from 0.5.x to 0.6.x.

### grep results

Only two `#[param(name = "...")]` occurrences exist in the entire workspace:

```
crates/normalize-budget/src/service.rs:310
crates/normalize-budget/src/service.rs:337
```

Both are in `BudgetService`, methods `measure` and `add`, for the same param:

```rust
#[param(name = "diff-ref", help = "Compute diff against this git ref")] base_ref: Option<String>
```

### Rename-risk table

| location | rust_identifier | kebab_case_of_identifier | declared_name | match? | severity |
|---|---|---|---|---|---|
| `BudgetService::measure` (`service.rs:310`) | `base_ref` | `base-ref` | `diff-ref` | **NO** | HIGH |
| `BudgetService::add` (`service.rs:337`) | `base_ref` | `base-ref` | `diff-ref` | **NO** | HIGH |

### Effect

| binary | flag shown | derivation |
|---|---|---|
| 0.5.x published binary | `--base-ref` | kebab-case of `base_ref` (name ignored) |
| 0.6.0 current binary | `--diff-ref` | honors `name = "diff-ref"` |

**The rename has already materialized.** The current binary (compiled against the 0.6.0 path
override in `Cargo.toml`) shows `--diff-ref` in `budget measure --help` and `budget add --help`.
Any scripts or CI configs written against the 0.5.x flag `--base-ref` are now silently broken —
`--base-ref` is unrecognized and silently dropped or errors.

### Status: 0 attributes are safe; 2 are mismatched

There are no `#[param(name = "...")]` annotations that are consistent with their field name —
the two that exist both differ, and both have already changed. There are no additional mismatches
to worry about from future annotations because there are no others.

**Verdict: the 0.6 rename already happened; `--base-ref` → `--diff-ref` in `budget measure` and
`budget add` is the only breaking rename.**

---

## 2. Cross-Command Naming Inconsistencies (MED)

### 2.1 `--ignore-case` vs `--case-insensitive` — same concept, different names

| command | flag |
|---|---|
| `normalize grep` | `-i, --ignore-case` |
| `normalize view` (and `view view`, `view list`, `view chunk`) | `-i, --case-insensitive` |
| `normalize view referenced-by`, `view references` | `-i, --case-insensitive` |
| `normalize view history`, `view trace` | `-i, --case-insensitive` |

Both flags mean "match case-insensitively." `grep` uses the unix-grep convention
(`--ignore-case`); `view` uses a more verbose form. A user who types
`normalize view --ignore-case` will get no match.

**Suggested canonical: `--ignore-case` / `-i` across the board** (matches `grep`, `rg`, `git grep`).

---

### 2.2 `--diff` vs `--diff-ref` vs `--baseline-ref` — git ref for comparison

Three different flag names are used to mean "compare metrics against this git ref":

| commands | flag | description in help |
|---|---|---|
| `rank complexity`, `rank ceremony`, `rank length`, `rank uniqueness`, `rank duplicates`, `rank density`, `rank imports`, `rank surface`, `rank depth-map`, `rank layering`, `rank files`, `rank coupling`, `rank ownership`, `rank test-ratio`, `rank budget` | `--diff` | "Show delta vs this git ref (branch, tag, commit, HEAD~N)" |
| `budget measure`, `budget add` | `--diff-ref` | "Compute diff against this git ref" |
| `ratchet measure` | `--diff-ref` | "Compute diff against this git ref (measure delta vs this ref)" |
| `ratchet check` | `--baseline-ref` | "Substitute this git ref as the baseline instead of the stored ratchet.json baseline" |
| `analyze skeleton-diff` | `[base]` positional | "Base ref to diff against (branch, commit, HEAD~N)" |

Five different forms for the same concept. The `rank` family is consistent with itself; the
`budget`/`ratchet` family uses `--diff-ref`; `ratchet check` deviates further to `--baseline-ref`;
`analyze skeleton-diff` takes it as a positional.

The `--diff-ref` form in budget/ratchet is also the result of the 0.6 rename (the Rust field was
`base_ref` → `--base-ref` in 0.5.x, now `--diff-ref` in 0.6.x via the `name` attribute).

**Suggested canonical: `--diff` everywhere for "show delta vs this ref" (read-only context),
`--diff-ref` for commands that need a separate ref for their computation (budget, ratchet
measure). `--baseline-ref` in `ratchet check` is semantically distinct enough to keep, but the
positional in `analyze skeleton-diff` should become `--base` or `--diff`.**

---

### 2.3 `--limit` / `-l` vs `-n, --limit` — inconsistent short form for the same flag

The vast majority of commands use `-l, --limit` to cap output results:

> `grep`, `analyze health`, `analyze summary`, `analyze docs`, `analyze architecture`,
> `analyze coupling-clusters`, `view history`, `view graph`, `view import-path`, `view blame`,
> `structure files`, `rank *` (all 23 subcommands), `rank call-complexity` (`-l` for per-list),
> `sessions plans`

But the bulk of the `sessions` subcommands use `-n, --limit`:

| sessions commands with `-n, --limit` |
|---|
| `sessions list`, `sessions stats`, `sessions ngrams`, `sessions messages`, `sessions parallelization`, `sessions heatmap`, `sessions cost` |

Additionally, `trend multi` and `trend complexity/length/density/test-ratio` use `-n, --snapshots`
(a different concept, but `-n` means something different there too).

The `-n` short form collides with its own usage within `sessions ngrams`, where `--n` (no short
form, see §2.4) is the ngram-size parameter:

```
sessions ngrams --n <n>       # n-gram size (2=bigram, 3=trigram)
sessions ngrams -n, --limit   # max sessions
```

**Suggested fix: use `-l, --limit` uniformly. In `sessions ngrams`, rename the ngram-size param
from `--n` to `--ngram` or `--size`, freeing `-n` so it can be dropped or re-mapped.**

---

### 2.4 `--n` is not a real flag name — looks like a typo

`sessions ngrams` exposes:

```
--n <n>    N-gram size (2=bigram, 3=trigram, etc.; default: 2)
```

A flag named `--n` (two hyphens, single letter) is unusual and looks like a mistake. It has no
short form because `-n` was pre-empted by `--limit`. The correct fix is to rename the param
(suggestion: `--ngram-size` or `--size`, short form `-s`). The current form is jarring and
likely confuses users.

**Severity: MED — actively confusing to read; low blast radius since it's a single command.**

---

### 2.5 `--limit` vs `--top` vs `--worst` — multiple names for "show N results"

| command | flag | meaning |
|---|---|---|
| most commands | `--limit` | cap result count |
| `sessions ngrams` | `--top` | top K most frequent n-grams |
| `sessions heatmap` | `--top` | top N files by write count |
| `rank density` | `-w, --worst` | number of worst files to show |

`--top` and `--worst` both mean "cap the display count" but use different flag names from
`--limit`. The `--worst` flag on `rank density` is particularly surprising — it means
"the N worst-scoring modules to include in the report" but the semantics are identical to
`--limit` everywhere else.

**Suggested canonical: `--limit` uniformly. Retire `--top` and `--worst` in favour of `--limit`.**

---

## 3. Positional vs Flag Inconsistencies (MED)

### 3.1 `view trace` uses `--target` for the file while most commands use positional

`view trace` signature:
```
normalize view trace [OPTIONS] [symbol]
  [symbol]      Symbol to trace (file/symbol or symbol name)
  -t, --target  Target file containing the symbol
```

Every other `view` subcommand accepts `[target]` as a positional for the full path+symbol form
(`path/Symbol`). `view trace` splits this into a positional symbol and a flag file, using
`--target` for the file. A user who types `normalize view trace src/lib.rs/MyFn` as a single
positional (the form that works in `view view`) will be confused.

Also, `-t` is the short form of `--target` in `view trace`, but `-t` is `--to` in `translate`,
`--type` in `rules run`/`rules list`, and `--threshold` in `rank complexity`. The same short
flag means four different things across four commands — acceptable since they're in separate
trees, but worth noting.

**Suggested fix: make `view trace` accept the same `path/symbol` positional form as `view view`
and drop the `--target` flag.**

---

### 3.2 `analyze skeleton-diff [base]` takes git ref as positional; all other ref comparisons use flags

`analyze skeleton-diff` signature:
```
normalize analyze skeleton-diff [OPTIONS] [base]
  [base]  Base ref to diff against (branch, commit, HEAD~N)
```

Every other command that takes a git ref uses a flag (`--diff`, `--diff-ref`, `--baseline-ref`).
This is the only command where the git ref is a positional argument. The naming also differs
(`base` vs the rest).

**Suggested fix: replace positional `[base]` with `--base` or `--diff` flag for consistency with
`rank` commands.**

---

### 3.3 `edit extract-function` and `context migrate` invert the dry-run default

All mutating commands in the normalize CLI write by default and offer `--dry-run` to preview:

```
edit delete --dry-run
edit replace --dry-run
edit swap --dry-run
edit rename --dry-run
edit move --dry-run
edit undo --dry-run
daemon add --dry-run
daemon remove --dry-run
grammars install --dry-run
generate types --dry-run
```

Two commands invert this convention:

| command | default | opt-in to write |
|---|---|---|
| `edit extract-function` | dry-run | `--apply` |
| `context migrate` | dry-run | `--apply` |

This means a user who forgets about `--apply` on `edit extract-function` but expects the
standard normalize convention will see a preview and wonder why nothing changed. The help text
says "By default this is a dry-run; pass `--apply` to write the changes" — but this
is the opposite of every other edit command.

**Suggested fix: make these commands write by default (consistent with the rest of the CLI) and
add `--dry-run` to preview. The `edit extract-function` case may have been an intentional
safety measure — if so, document it as a deliberate exception.**

---

## 4. Other Inconsistencies (LOW)

### 4.1 `-d` short form means three different things

| context | `-d` means |
|---|---|
| `view view`, `view list`, `package tree`, `syntax ast` | `--depth` |
| `view trace` | `--max-depth` |
| `structure test-fixtures`, `rules test-fixtures` | `--fixture-dir` |

Within the `view` tree, `-d` is `--depth` in most subcommands but `--max-depth` in `view trace`.
In the `structure`/`rules test-fixtures` family, `-d` means the fixture directory path.
Users who muscle-memory `-d` for depth will accidentally pass it in `test-fixtures` contexts.

**Suggested fix: use a distinct short form for `--fixture-dir` (e.g., nothing, or `--dir`).**

---

### 4.2 `syntax ast --compact` has a missing description

```
normalize syntax ast --compact    "Enable compact"
```

Every other command describes `--compact` as:
```
Compact output without colors (overrides TTY detection)
```

The `syntax ast` description is truncated. Likely a copy-paste or generation artifact.

---

### 4.3 `-l` short form means `--limit` everywhere except `syntax ast`

In `syntax ast`:
```
-l, --at-line <at-line>    Show node at specific line
```

In every other command that has `-l`, it means `--limit`. A user who types
`normalize syntax ast src/main.rs -l 100` expecting to limit output depth will instead
jump to line 100.

**Suggested fix: drop the `-l` short form from `--at-line` (or remap it to something less
conflicting like `-L`).**

---

### 4.4 `view blame --sessions` — unusual flag name for session directory override

```
normalize view blame --sessions <sessions>    Override session directory
```

This is the only command with a `--sessions` flag (not to be confused with the `sessions`
top-level command). Its name suggests "enable sessions analysis" not "set sessions directory."
A more communicative name would be `--sessions-dir`.

---

### 4.5 `sessions ngrams --n` short-form collision with `-n, --limit`

Already detailed in §2.3 and §2.4. Noting here as a compounding inconsistency: the `--n`
flag for ngram size has no `-n` short form because `-n` is occupied by `--limit`, which
means `sessions ngrams` has both `--n` (long-only, unusual) and `-n` (short for something
else). This is a user-hostile interface surface.

---

### 4.6 `--module-limit` in `rank call-complexity` alongside `-l, --limit`

`rank call-complexity` has two distinct limit-like flags:
```
-l, --limit <limit>              Maximum functions to show per list (default: 20)
-m, --module-limit <module-limit>  Maximum number of modules to show (0=no limit)
```

The asymmetry (`--limit` for one table, `--module-limit` for another table in the same report)
is mildly confusing. A more explicit naming like `--function-limit` / `--module-limit` would
be clearer, but this is low priority.

---

## Summary Table

| # | finding | commands affected | severity |
|---|---|---|---|
| 1 | `base_ref` → `--base-ref` (0.5.x) to `--diff-ref` (0.6.x) — ALREADY CHANGED | `budget measure`, `budget add` | **HIGH** |
| 2 | `--ignore-case` (grep) vs `--case-insensitive` (view family) | `grep`, all `view` subcommands | MED |
| 3 | `--diff` vs `--diff-ref` vs `--baseline-ref` vs positional `[base]` for git ref | `rank *`, `budget *`, `ratchet check`, `analyze skeleton-diff` | MED |
| 4 | `-l` vs `-n` as short form for `--limit` | sessions commands vs all others | MED |
| 5 | `--n` as long-form-only flag name for ngram size | `sessions ngrams` | MED |
| 6 | `--limit` vs `--top` vs `--worst` | `sessions ngrams`, `sessions heatmap`, `rank density` | MED |
| 7 | `view trace --target` vs positional target in all other `view` subcommands | `view trace` | MED |
| 8 | `analyze skeleton-diff [base]` positional vs flags everywhere else | `analyze skeleton-diff` | MED |
| 9 | `--apply` opt-in inverts the write-by-default convention | `edit extract-function`, `context migrate` | MED |
| 10 | `-d` means `--depth`, `--max-depth`, and `--fixture-dir` | `view trace`, `*-test-fixtures` | LOW |
| 11 | `syntax ast --compact` description is truncated | `syntax ast` | LOW |
| 12 | `-l` means `--at-line` in `syntax ast`, `--limit` everywhere else | `syntax ast` | LOW |
| 13 | `--sessions` flag name ambiguous | `view blame` | LOW |
| 14 | `--n` / `-n` conflict in `sessions ngrams` | `sessions ngrams` | LOW (symptom of #4+#5) |
| 15 | `--limit` vs `--module-limit` in same report | `rank call-complexity` | LOW |
