# Dry-run Coverage Audit — 2026-06-29

**Scope:** Every mutating CLI command in `normalize`. Audited against the hard constraint in CLAUDE.md:
> "Ship mutating commands without --dry-run" is FORBIDDEN.

**Method:** `cargo build` → enumerate full command tree → `--help` grep for `--dry-run`/`--apply` → source confirmation for borderline cases.

---

## Hard-Constraint Violations

Commands that mutate state and have NO dry-run (or equivalent preview) mechanism.

| Command | Mutates what | Has --dry-run? |
|---|---|---|
| `edit redo` | Source files via shadow git | **MISSING** |
| `rules run --fix` | Source files (applies auto-fixes) | **MISSING** |
| `rules add` | Downloads & writes rule files to `.normalize/rules/` | **MISSING** |
| `rules update` | Overwrites imported rule files | **MISSING** |
| `rules remove` | Deletes imported rule files | **MISSING** |
| `rules setup` | Writes rule enable/disable config | **MISSING** |
| `structure rebuild` | Writes/replaces `.normalize/index.sqlite` | **MISSING** |
| `structure packages` | Writes to global grammar cache | **MISSING** |
| `kg write` | Writes/deletes units in knowledge graph (`.normalize/kg/`) | **MISSING** |
| `sessions mark` | Appends to `.normalize/sessions-reviewed` | **MISSING** |
| `sessions unmark` | Removes from `.normalize/sessions-reviewed` | **MISSING** |
| `update` | Downloads and installs new `normalize` binary in-place | **MISSING** (has `--check` but not `--dry-run`) |
| `ratchet add` | Writes baseline entry to ratchet config | **MISSING** |
| `ratchet update` | Overwrites ratchet baseline values | **MISSING** |
| `ratchet remove` | Removes ratchet baseline entry | **MISSING** |
| `budget add` | Writes budget entry to config | **MISSING** |
| `budget update` | Overwrites budget config entry | **MISSING** |
| `budget remove` | Removes budget config entry | **MISSING** |
| `daemon start` | Starts background daemon process | **MISSING** |
| `daemon stop` | Kills background daemon process | **MISSING** |
| `generate client` | Generates API client code (when `-o` provided) | **MISSING** |
| `generate cli-snapshot` | Generates test file (when `-o` provided) | **MISSING** |

**Total hard-constraint violations: 22 commands**

### Notes on borderline cases

- **`update`**: Has `--check` (checks without installing) but this is a separate mode, not `--dry-run`. The constraint calls for `--dry-run` specifically (show what would change before doing it).
- **`structure rebuild`** and **`structure packages`**: These are idempotent maintenance operations with no meaningful "preview" (the output IS the side effect). However, CLAUDE.md draws no exception for idempotent commands.
- **`generate client`** and **`generate cli-snapshot`**: Default to stdout. Only write files when `-o` is passed. Stdout output is a functional dry-run; the issue is there's no `--dry-run` flag to explicitly signal intent when `-o` is given.
- **`daemon start/stop`**: These control system processes, not files. A `--dry-run` would show what would happen. Borderline; included because the constraint doesn't carve out an exception.
- **`sessions mark/unmark`**: Small metadata-only mutations. Still write a file.
- **`kg write` with null transform (`normalize kg write my-id 'null'`)**: Destructive deletion. Especially important case for `--dry-run`.

---

## Commands WITH --dry-run (or safe-by-default --apply pattern)

| Command | Dry-run mechanism | Notes |
|---|---|---|
| `init` | `--dry-run` | Previews config file creation |
| `edit delete` | `--dry-run` | Previews symbol deletion |
| `edit replace` | `--dry-run` | Previews symbol replacement |
| `edit swap` | `--dry-run` | Previews symbol swap |
| `edit insert` | `--dry-run` | Previews content insertion |
| `edit rename` | `--dry-run` | Previews rename across files |
| `edit undo` | `--dry-run` | Previews undo |
| `edit goto` | `--dry-run` | Previews shadow history jump |
| `edit batch` | `--dry-run` | Previews batch edits |
| `edit move` | `--dry-run` | Previews symbol move + import rewrite |
| `edit introduce-variable` | `--dry-run` | Previews extraction |
| `edit inline-variable` | `--dry-run` | Previews inline |
| `edit add-parameter` | `--dry-run` | Previews signature change + call site updates |
| `edit inline-function` | `--dry-run` | Previews function inline |
| `edit extract-function` | `--apply` pattern | Default is dry-run; must pass `--apply` to write |
| `rules enable` | `--dry-run` | Previews config change |
| `rules disable` | `--dry-run` | Previews config change |
| `sync` | `--dry-run` | Previews what would be copied |
| `daemon add` | `--dry-run` | Previews adding a watch root |
| `daemon remove` | `--dry-run` | Previews removing a watch root |
| `grammars install` | `--dry-run` | Previews grammar downloads |
| `config set` | `--dry-run` | Previews config file change |
| `generate types` | `--dry-run` | Previews generated output without writing |
| `context migrate` | `--apply` pattern | Default is dry-run preview; must pass `--apply` to migrate |

**Confirmed safe: 24 commands**

---

## Inconsistency: `edit undo` vs `edit redo`

`edit undo` has `--dry-run`. `edit redo` does not. These are symmetric operations (both apply shadow git commits to working tree). The asymmetry appears to be a simple oversight — confirmed by inspecting source: `redo()` calls `do_undo_redo(..., dry_run: false)` with no way to pass `true` from the CLI.

---

## Broken/Incomplete Dry-runs

None confirmed. All `--dry-run` implementations reviewed show:
- `edit` commands: call the mutation path only when `dry_run = false`
- `config set`: checks schema, builds diff, skips write when `dry_run = true`
- `daemon add/remove`: returns a preview message without touching the watch list
- `grammars install`: queries release manifest without downloading
- `generate types`: skips file write, prints to stdout

No false dry-runs (where `--dry-run` is claimed but mutation still occurs) were found.

---

## Dangerous Mutate-by-Default Commands

No command was found that mutates **without any explicit trigger** — every mutating command either requires a positional argument, a URL, or a flag like `--fix`. However:

- **`kg write`** deletes a unit when the jq transform returns `null`. The help text explains this but there's no confirmation or dry-run.
- **`structure rebuild`** with `--full` destroys and rebuilds the entire index. No preview of what would be cleared.
- **`rules run --fix`** applies fixes to every matching file in a directory. No preview, no per-file confirmation.

---

## Read-only Commands (Denominator)

| Service | Read-only commands |
|---|---|
| `grep` | all (pattern search only) |
| `view` | all |
| `structure` | `stats`, `files`, `query`, `test-fixtures` |
| `edit` | `history` |
| `rules` | `list`, `show`, `tags`, `validate`, `compile`, `test`, `test-fixtures` |
| `kg` | `read`, `walk` |
| `ci` | all (runs checks, does not write) |
| `analyze` | all |
| `rank` | all |
| `trend` | all |
| `budget` | `measure`, `check`, `show` |
| `cfg` | all |
| `ratchet` | `measure`, `check`, `show` |
| `aliases` | all |
| `translate` | read-only when no `-o` (stdout only) |
| `docs` | all |
| `context` | default (dry-run preview) |
| `guide` | all |
| `generate` | `cli-snapshot` when no `-o`; `types` when no `-o` |
| `package` | all |
| `sessions` | `list`, `show`, `analyze`, `stats`, `ngrams`, `messages`, `subagents`, `patterns`, `parallelization`, `heatmap`, `cost`, `plans` |
| `daemon` | `status`, `watch`, `list` |
| `grammars` | `list`, `paths` |
| `syntax` | all |
| `tools` | `lint`, `test` (runs external tools, reads output) |
| `config` | `schema`, `show`, `validate` |
| `serve` | all (starts server, no disk writes) |

---

## Summary

| Category | Count |
|---|---|
| **Mutating commands MISSING --dry-run (violations)** | **22** |
| Mutating commands WITH --dry-run | 24 |
| Read-only commands (approximate) | ~60+ |
| Total named subcommands (approximate) | ~110+ |

### Priority order for adding --dry-run

1. **`edit redo`** — asymmetric with `edit undo`; one-line fix
2. **`rules run --fix`** — highest blast radius; applies fixes across entire codebase
3. **`kg write` with null** — destructive deletion, no confirmation
4. **`rules add/update/remove`** — modifies rule configuration without preview
5. **`ratchet add/update/remove`** and **`budget add/update/remove`** — config mutations
6. **`rules setup`** — wizard writes config; should at least print what it would change
7. **`sessions mark/unmark`** — low severity; small metadata files
8. **`structure rebuild/packages`** — idempotent but still writes; low-priority

### Commands where `--dry-run` is less critical (but still violates the constraint)

- `daemon start/stop` — system process control; preview would say "would start daemon at X"
- `update` — `--check` fills most of the need; gap is minor
- `generate client/cli-snapshot` — stdout-by-default pattern already provides a natural preview
