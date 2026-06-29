# CLI Command Structure Audit — 2026-06-29

Audit performed against `/home/me/git/rhizone/normalize/target/debug/normalize`
(debug build, current master). Every service `--help` and every subcommand `--help`
was collected and analyzed. Read-only audit — no commits made.

---

## Complete Command Tree

```
normalize
├── Core
│   ├── grep                           leaf — ripgrep pattern search
│   ├── init                           leaf — project initialization
│   ├── view                           service (target positional + subcommands)
│   │   ├── view                       redundant explicit form of positional
│   │   ├── chunk                      paginate large files
│   │   ├── referenced-by              callers in call graph (index required)
│   │   ├── list                       list code entities
│   │   ├── references                 callees in call graph (index required)
│   │   ├── history                    git history for file/symbol
│   │   ├── dependents                 reverse-import closure (index required)
│   │   ├── trace                      value provenance
│   │   ├── graph                      graph-theoretic properties (cycles, hubs)
│   │   ├── import-path                shortest import chain (index required)
│   │   └── blame                      git blame + session attribution
│   ├── structure                      service
│   │   ├── rebuild                    build index
│   │   ├── stats                      index statistics
│   │   ├── files                      list indexed files
│   │   ├── packages                   index external packages
│   │   ├── query                      raw SQL against index
│   │   └── test-fixtures              language extraction fixture tests
│   ├── edit                           service
│   │   ├── history                    service (shadow git)
│   │   │   ├── list
│   │   │   ├── diff
│   │   │   ├── status
│   │   │   ├── tree
│   │   │   └── prune
│   │   ├── delete
│   │   ├── replace
│   │   ├── swap
│   │   ├── insert
│   │   ├── rename
│   │   ├── undo
│   │   ├── redo
│   │   ├── goto
│   │   ├── batch
│   │   ├── move
│   │   ├── introduce-variable
│   │   ├── inline-variable
│   │   ├── add-parameter
│   │   ├── inline-function
│   │   └── extract-function
│   ├── rules                          service
│   │   ├── list
│   │   ├── run
│   │   ├── enable
│   │   ├── disable
│   │   ├── show
│   │   ├── tags
│   │   ├── add
│   │   ├── update
│   │   ├── remove
│   │   ├── setup
│   │   ├── validate
│   │   ├── compile
│   │   ├── test
│   │   └── test-fixtures
│   └── kg                             service — knowledge graph
│       ├── read
│       ├── write
│       └── walk
│
├── Analysis
│   ├── ci                             leaf — run all checks
│   ├── analyze                        service (target positional + subcommands)
│   │   ├── health
│   │   ├── all
│   │   ├── summary
│   │   ├── liveness
│   │   ├── effects
│   │   ├── exceptions
│   │   ├── docs                       (Repository category)
│   │   ├── architecture               (Graph analysis category)
│   │   ├── coupling-clusters          (Git history category)
│   │   ├── activity                   (Git history category)
│   │   ├── repo-coupling              (Git history category)
│   │   ├── cross-repo-health          (Git history category)
│   │   ├── security                   (Security category)
│   │   └── skeleton-diff              (Diff category)
│   ├── rank                           service (no positional; categorized subcommands)
│   │   ├── Code quality
│   │   │   ├── complexity
│   │   │   ├── ceremony
│   │   │   ├── length
│   │   │   ├── uniqueness
│   │   │   ├── call-complexity
│   │   │   ├── duplicates
│   │   │   ├── duplicate-types
│   │   │   └── fragments
│   │   ├── Module structure
│   │   │   ├── size
│   │   │   ├── density
│   │   │   ├── imports
│   │   │   ├── surface
│   │   │   ├── depth-map
│   │   │   ├── layering
│   │   │   └── module-health
│   │   ├── Repository
│   │   │   └── files
│   │   ├── Git history
│   │   │   ├── hotspots
│   │   │   ├── coupling
│   │   │   ├── ownership
│   │   │   └── contributors
│   │   └── Testing
│   │       ├── test-ratio
│   │       ├── test-gaps
│   │       └── budget          *** NAME CLASH with top-level `budget` service ***
│   ├── trend                          service
│   │   ├── multi
│   │   ├── complexity
│   │   ├── length
│   │   ├── density
│   │   └── test-ratio
│   ├── budget                         service — diff budget CRUD
│   │   ├── measure
│   │   ├── add
│   │   ├── check
│   │   ├── update
│   │   ├── show
│   │   └── remove
│   ├── cfg                            service (ONE subcommand only)
│   │   └── cfg                        *** REDUNDANT WRAPPING — `normalize cfg cfg` ***
│   └── ratchet                        service
│       ├── measure
│       ├── add
│       ├── check
│       ├── update
│       ├── show
│       └── remove
│
├── Utilities
│   ├── aliases                        leaf
│   ├── translate                      leaf
│   ├── docs                           leaf
│   ├── sync                           leaf
│   ├── context                        service
│   │   └── migrate
│   ├── guide                          service
│   │   ├── rules
│   │   ├── explore
│   │   ├── setup
│   │   ├── analyze            *** STALE CONTENT — references pre-rename command paths ***
│   │   └── tree-sitter
│   ├── generate                       service
│   │   ├── client
│   │   ├── types
│   │   └── cli-snapshot
│   ├── package                        service
│   │   ├── info
│   │   ├── list
│   │   ├── tree
│   │   ├── why
│   │   ├── outdated
│   │   └── audit
│   └── sessions                       service
│       ├── list
│       ├── show
│       ├── analyze
│       ├── stats
│       ├── ngrams
│       ├── messages
│       ├── subagents
│       ├── patterns
│       ├── parallelization
│       ├── heatmap
│       ├── cost
│       ├── plans
│       ├── mark
│       └── unmark
│
└── Infrastructure
    ├── update                         leaf
    ├── daemon                         service
    │   ├── status
    │   ├── stop
    │   ├── start
    │   ├── run
    │   ├── add
    │   ├── remove
    │   ├── watch
    │   └── list
    ├── grammars                       service
    │   ├── list
    │   ├── install
    │   └── paths
    ├── syntax                         service
    │   ├── ast
    │   ├── query
    │   └── node-types
    ├── tools                          service
    │   ├── lint                       service
    │   │   ├── run
    │   │   └── list
    │   └── test                       service
    │       ├── run
    │       └── list
    ├── config                         service
    │   ├── schema
    │   ├── show
    │   ├── validate
    │   └── set
    └── serve                          service
        ├── mcp
        ├── http
        └── lsp
```

Total top-level services: 30  
Total leaf commands (approximate): ~165

---

## Defect Inventory

### HIGH — Broken/Stub/Duplicate

---

**H-1: `normalize cfg cfg` — redundant double-wrapping**

`cfg` is a service with exactly ONE subcommand, also named `cfg`. The only way to
use the feature is `normalize cfg cfg <path>`. The service level adds zero value and
forces the user to type the verb twice. The parent help text already says "Build and
render the control flow graph" — identical to the subcommand's description.

Fix: collapse `cfg cfg` into just `cfg` (make it a direct command, not a service).
The service wrapper exists with no benefit.

---

**H-2: `normalize rank budget` name collision with `normalize budget`**

Two completely different features share the word "budget":

- `normalize rank budget` — "Break down line counts by purpose: business logic, tests,
  docs, config, and generated code." (a classification/ranking command)
- `normalize budget` — CRUD enforcement of PR diff size limits (add/check/show/remove)

These are unrelated concepts. A user looking for "budget" gets two hits that mean
entirely different things. `rank budget` should be renamed: `rank line-breakdown`,
`rank categories`, or `rank purposes` are all less ambiguous.

---

**H-3: `normalize kg --help` examples reference six non-existent commands**

The parent-level help for `kg` ends with:
```
normalize kg create --id my-design --metadata tag=design
normalize kg link --from my-design --to api-spec --kind references
normalize kg query --match tag=design
normalize kg show my-design
```

None of these commands exist. The actual subcommands are `kg read`, `kg write`,
`kg walk`. This is stale copy from a previous API that was replaced. A new user
following these examples gets immediate errors.

---

**H-4: `normalize guide analyze` body references ~6 commands that do not exist**

Running `normalize guide analyze` outputs a guide with these command examples:

```
normalize analyze all                   # everything at once
normalize analyze health                # quick health check
normalize analyze summary               # generated overview
normalize analyze complexity            # DOES NOT EXIST
normalize analyze length                # DOES NOT EXIST
normalize analyze duplicates            # DOES NOT EXIST
normalize analyze duplicates --scope blocks  # DOES NOT EXIST
normalize analyze ceremony              # DOES NOT EXIST
normalize analyze size                  # DOES NOT EXIST
normalize analyze density               # DOES NOT EXIST
```

These commands were moved from `analyze` to `rank` at some point (the guide
references the old paths). The correct commands are `normalize rank complexity`,
`normalize rank length`, `normalize rank duplicates`, etc. Any user following
this guide gets 7 "error: unrecognized subcommand" responses out of ~10 examples.

---

**H-5: `normalize guide rules` body references `normalize analyze node-types`**

Running `normalize guide rules` outputs:
```
normalize analyze node-types python --search raise
```

The actual command is `normalize syntax node-types`. `analyze` has no `node-types`
subcommand. Another stale reference from the same era as H-4.

---

### MEDIUM — Misplaced/Inconsistent/Overlapping

---

**M-1: `analyze architecture` substantially overlaps with `view graph`**

- `normalize analyze architecture`: "Detects circular imports, highly-coupled module
  pairs, and hub modules." Returns an `ArchitectureReport`.
- `normalize view graph`: "Reports dependency cycles (circular imports), hub modules
  (high fan-in/fan-out), and graph centrality." Returns a `GraphReport`.

Both commands:
- Require the facts index
- Detect circular imports / dependency cycles
- Find hub modules (high fan-in/fan-out)

The difference is that `analyze architecture` adds coupling pairs while `view graph`
adds centrality scores. From a user perspective these are the same feature at different
detail levels. Neither help text distinguishes them or mentions the other exists.

Either: (a) merge into one command, or (b) add cross-references in each help text
explaining when to use which.

---

**M-2: `edit extract-function` has inverted dry-run default**

Every other `edit` command defaults to writing and requires `--dry-run` to preview:
```
normalize edit delete ... --dry-run
normalize edit rename ... --dry-run
normalize edit move   ... --dry-run
```

But `normalize edit extract-function` defaults to dry-run and requires `--apply` to
actually write:
```
normalize edit extract-function ... --apply
```

The flag is documented in the help text ("By default this is a dry-run; pass `--apply`
to write the changes") but it violates the uniform convention of every other command in
the service. A user who forgets `--apply` will think nothing happened.

---

**M-3: `normalize edit history` vs `normalize view history` — same name, different concepts**

- `normalize edit history` — shadow git edit history (the undo/redo log; list/diff/status/tree/prune)
- `normalize view history` — git history for a file or symbol (commit log)

Users navigating by instinct will try the wrong one first. The `edit history` concept
should probably be named `edit log` (or `edit trail`) since "history" in CLI convention
typically refers to git history, not an application-level undo log.

---

**M-4: `normalize analyze all` — no body description, opaque scope**

`normalize analyze all --help` shows usage and options but no description paragraph
explaining what "all" runs. Running it (`normalize analyze all .`) produces a "Codebase
Health" report indistinguishable from `normalize analyze health`. It is unclear whether
`analyze all` runs more passes than `analyze health` or is simply an alias.

The CLAUDE.md principle "Non-interactive != non-functional" and the general quality bar
require that the scope of `analyze all` be documented and distinguishable.

---

**M-5: `normalize syntax ast --compact` flag name clashes with global `--compact`**

`normalize syntax ast` takes `--compact` meaning "show node-type outline, no source
text" (a display mode for the AST). The global `--compact` means "compact output without
colors (overrides TTY detection)." Both flags appear in `syntax ast --help`. Users
cannot tell which behavior they are getting, and both flags apply simultaneously.

The AST-specific flag should be named differently (e.g. `--outline`).

---

**M-6: `analyze coupling-clusters` and `rank coupling` cover the same domain without
cross-reference**

- `analyze coupling-clusters` — groups files into clusters by temporal coupling
  (connected components, BFS)
- `rank coupling` — ranks file PAIRS by temporal coupling count

Same underlying data (git commit co-occurrence), different aggregations. Neither
mentions the other. A user exploring temporal coupling will find one or the other
depending on search order and may not discover the complementary view.

---

### LOW — Help-Text Polish

---

**L-1: `normalize rules remove` has no examples section**

Every peer command in `rules` (enable, disable, show, tags, compile, test) has
examples. `rules remove` does not.

---

**L-2: `normalize rules update` has no examples section**

Same issue as L-1. `rules update` omits examples. The command also has no
description body paragraph (only the one-line summary).

---

**L-3: `analyze activity`, `analyze repo-coupling`, `analyze cross-repo-health` have
no examples sections**

Three of the git-history analysis commands under `analyze` omit the examples block
that every comparable command provides.

---

**L-4: `normalize analyze all` no body text**

In addition to M-4 (opaque scope), the help text has no body paragraph at all —
only usage + arguments + options. Contrast with `analyze health` which has a
multi-sentence description. At minimum a one-liner explaining what passes `all` runs
should be present.

---

**L-5: Intra-`analyze` categories not visible in help**

The `normalize analyze --help` shows categories inline in the subcommand listing
(Repository, Graph analysis, Git history, Security, Diff) but these do not appear
in the service header or navigation cue. The category tags appear only as labels
in the subcommand list with no blank-line separation, making them hard to scan.

---

## Root-Global Flag Noise

**Every single leaf command (~165 total) displays the following flags in its `--help`
output regardless of relevance:**

```
--pretty
--compact
--json
--jsonl
--jq
--input-schema
--output-schema
--manual
--params-json
```

These are server-less-generated global flags that appear identically on every command
at every depth level. They occupy roughly 9 lines in every `--help` output — taking up
more space than the actual command-specific flags in many cases (e.g. `daemon status`,
`kg walk`, `guide rules`, `aliases`). The global flags also appear on the root `normalize
--help`, so they repeat in full at every level.

Count: all 165+ leaf commands are affected (100%).

This is the highest-volume discoverability issue in the CLI. A user reading `normalize
daemon status --help` sees 9 infrastructure flags they cannot practically configure on
that command before they see the single actual flag (`--json`-family) relevant to their
query.

This is a server-less rendering concern. The flags are technically accurate (they do
work on every command) but they bury command-specific signals. The fix is either:
(a) collapse them into a `[global options]` footer in server-less rendering, or
(b) omit them from leaf command help and note "run `normalize --help` for global flags."

---

## Coherence Summary

The command tree has three tiers of quality:

**Well-designed services:** `edit`, `rules`, `structure`, `syntax`, `config`, `ratchet`,
`budget`, `sessions`, `package`, `daemon`, `grammars`. These have clean verb-noun
subcommand shapes, consistent flag patterns, and good examples coverage.

**Services with coherence gaps:** `analyze` and `rank` have a blurry boundary — both
contain git-history analysis, both contain structural quality metrics. The categories
within each service repeat (`rank` has its own "Testing" and "Git history" sections;
`analyze` also has "Git history" and "Repository" sections). The guide stale references
(H-4, H-5) prove that commands were moved between these services without updating
downstream documentation. The `analyze`/`rank` split would benefit from a written
rationale (in docs/) distinguishing what belongs where.

**Structurally wrong:** `cfg` (H-1), `rank budget` naming (H-2), and `kg` examples (H-3)
are the three cleanest bugs with obvious fixes.

**Guide system is broken:** Two of the five guides (`analyze`, `rules`) reference commands
that don't exist. The guide system's static content has no test coverage (unlike
`structure test-fixtures` and `rules test-fixtures` which self-test the extraction and
rules engines). A `guide test` or snapshot-test against guide output would have caught H-4
and H-5 before they shipped.
