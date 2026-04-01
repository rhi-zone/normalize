# CLI Coherence Audit — 2026-04-01

## Framing

Normalize is a code intelligence OS. An OS needs a consistent UX language — not 22
independent commands that each feel like a different tool. This doc captures the
inconsistencies, gaps, and confusion points found in an adversarial audit of the
current CLI surface.

## Current Command Surface (22 top-level commands)

```
grep       view       edit       translate   generate
analyze    rank       trend      budget      ratchet
rules      ci         structure  syntax      package
sessions   tools      daemon     grammars    serve
init       update     config     context     aliases    guide
```

## Problem 1: Naming Inconsistency

**Verb commands** (action-first): `grep`, `view`, `edit`, `translate`, `generate`,
`analyze`, `rank`, `trend`, `serve`, `init`, `update`

**Noun commands** (object-first): `structure`, `syntax`, `package`, `sessions`,
`tools`, `daemon`, `grammars`, `rules`, `config`, `context`, `aliases`, `guide`,
`budget`, `ratchet`, `ci`

No consistent convention. Noun commands tend to have sub-verbs (`rules run`,
`structure rebuild`) while verb commands stand alone or have noun subcommands
(`view referenced-by`). Not wrong per se, but the inconsistency means a user
can't predict the shape of a command from the pattern.

## Problem 2: Overlapping Commands

### Search (4 commands, unclear when to use which)

| Command | What it does | Requires |
|---------|-------------|----------|
| `grep` | Text pattern search (ripgrep) | Nothing |
| `view` + path | File/symbol browsing | Nothing |
| `view referenced-by` | Symbol usage lookup | Index |
| `structure search` | Semantic search by meaning | Index + embeddings |
| `syntax query` | AST pattern matching | Nothing |

A user who wants "find all uses of X" has no guidance on which to pick.

### Analysis (3 commands, fuzzy boundaries)

| Command | What it does |
|---------|-------------|
| `analyze` | Static analysis (40+ subcommands: health, architecture, security, docs, duplicates...) |
| `rank` | Sorted metric lists (complexity, coupling, hotspots, duplicates, test gaps...) |
| `trend` | Metrics over git history |

`analyze` and `rank` overlap heavily. `rank complexity` vs `analyze complexity` —
what's the difference? `rank` returns sorted lists, `analyze` returns summaries?
The mental model is unclear.

### Metric tracking (2 commands, similar purpose)

| Command | What it does |
|---------|-------------|
| `budget` | Diff-based limits (how much can change per commit) |
| `ratchet` | Regression tracking with a baseline |

Both answer "is this metric getting worse?" with different mechanisms.

### Configuration (4 commands, scattered)

| Command | What it does |
|---------|-------------|
| `config` | TOML inspection/validation |
| `rules` | Rule enable/disable/run |
| `context` | Context hierarchy |
| `aliases` | Filter alias management |

All are config-adjacent but live at the top level.

## Problem 3: Missing Expected Commands

Users from other tools will try these and get nothing useful:

- `normalize search` — suggests `serve` (useless)
- `normalize find` — suggests `serve` (useless)
- `normalize lint` — not found
- `normalize check` — not found
- `normalize index` — not found
- `normalize refactor` — not found

These should either be aliases or produce helpful "did you mean?" suggestions.

## Problem 4: Silent Failures / Bad Prerequisites

- `view referenced-by X` without index: fails silently or returns empty
- `structure search` without embeddings: returns empty with a hint, but only
  if configured wrong — if embeddings just haven't been populated yet, no hint
- `view /nonexistent/path` doesn't error — treats it as a symbol pattern and
  matches something unrelated
- `rules run` silently backgrounds with no output

**Principle violation**: CLAUDE.md says "non-interactive != non-functional" and
"never silently return empty results." Several commands violate this.

## Problem 5: No Composition

Commands are silos. Common desired workflows:

1. **Find → Fix**: `rank complexity` finds bad functions → no way to pipe into `edit`
2. **Analyze → Act**: `analyze duplicates` finds clones → no merge suggestion
3. **Search → Navigate**: `grep` finds matches → no way to expand to symbol context
4. **Rule → Fix**: `rules run` finds violations → `--fix` exists for some but not all

An OS has pipes. Normalize commands don't compose.

## Problem 6: Cognitive Overload

22 top-level commands is too many. Unix has hundreds of commands but you learn
5-10 and reach for them daily. Normalize doesn't have that hierarchy — everything
is presented equally, with no indication of what's core vs. niche.

**Core commands** (what 90% of users need 90% of the time):
- `view` — understand code
- `grep` — find code
- `edit` — change code
- `rules` — check code
- `structure` — build the index

**Supporting commands** (useful but not daily):
- `analyze`, `rank`, `trend` — deeper analysis
- `budget`, `ratchet` — metric tracking
- `package` — dependency info
- `sessions` — agent log analysis

**Infrastructure** (rarely invoked directly):
- `daemon`, `grammars`, `config`, `aliases`, `context`, `serve`
- `init`, `update`, `ci`

These tiers aren't reflected in the CLI at all. Every command gets equal billing.

## Problem 7: No Guided Path

A new user runs `normalize --help` and sees 22 commands. There's no:
- "Start here" indicator
- Progressive disclosure (show core commands first)
- Decision tree ("want to find code? try grep. want to understand structure? try view")
- Post-command suggestions ("Found 42 complex functions. Try: rank complexity --top 10")

`normalize guide` exists but is buried in the list. `normalize init` sets up config
but doesn't build the index or explain what to do next.

## Resolved

### Nesting: mostly rejected

Investigation found that most proposed groupings are **forced** — commands that look
overlapping from the outside serve genuinely different purposes:
- `budget` vs `ratchet`: different enforcement mechanisms (per-diff vs baseline)
- `daemon` vs `serve`: different lifespans and purposes
- `sessions` under `analyze`: different domain entirely (log analysis vs code analysis)
- `config` vs `aliases` vs `context`: different storage and semantics

Only `context` under `config` was natural; everything else stays top-level.

### analyze / rank / trend: same thing, should unify

All three compute metrics; they differ only in output format. `rank complexity`
literally calls `analyze::complexity::analyze_codebase_complexity()`. These should
become one command with output mode flags. Design TBD.

### Command aliases: done (b89df0b2)

`search`/`find` → `grep`, `lint` → `rules run`, `check` → `ci`,
`index` → `structure rebuild`, `refactor` → `edit`.

### Auto-rebuild index: done (0f9f932e)

Commands that need the index auto-build it if missing. Clear error if disabled.

## Remaining

- Tiered help (core commands first, `--help-all` for everything)
- Post-command suggestions ("Found 42 complex functions. Try: ...")
- analyze/rank/trend unification design
- Command composition (piping analysis output into edit)
- Help text quality pass ("when to use this" not just "what this does")
