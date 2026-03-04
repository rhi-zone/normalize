# Unification Decisions

This document tracks decisions where we chose unified approaches over specialized ones. The goal: avoid runaway complexity by composing from fewer primitives.

See `docs/philosophy.md` for the "Generalize, Don't Multiply" tenet.

## Core Commands: view, edit, analyze

**Decision**: Three composable primitives instead of many specialized commands.

**Alternatives rejected**:
- `normalize show-imports`, `normalize show-deps`, `normalize show-callees`, `normalize show-symbols` → unified under `view` with flags
- `normalize health`, `normalize complexity`, `normalize security`, `normalize hotspots` → unified under `analyze` with flags
- `normalize edit-function`, `normalize rename-symbol`, `normalize add-import` → unified under `edit` with structural targeting

**Why**: Each new command adds cognitive load. Flags on a core command are discoverable via `--help`. Same path resolution, filtering, and output formatting applies everywhere.

**Trade-off**: Flags can be cryptic. Mitigation: `--json` + `--jq` for programmatic use, good defaults for interactive use.

## Path Resolution: dwim.rs

**Decision**: Single path resolver for files, directories, symbols, and fuzzy matches.

**Alternatives rejected**:
- Separate resolution for `view src/` vs `view MyClass` vs `view MyClass.method`
- Explicit prefixes like `view file:src/` vs `view symbol:MyClass`

**Why**: Users think in terms of "show me X", not "resolve X as a file path and then show it". The resolver tries interpretations in order (literal path, indexed symbol, fuzzy match) and picks the best.

**Trade-off**: Ambiguity when a symbol and file have the same name. Mitigation: `path/symbol` syntax for disambiguation.

## Output Formatting: --json + --jq

**Decision**: JSON output with jq filtering instead of many format flags.

**Alternatives rejected**:
- `--format=json`, `--format=yaml`, `--format=csv`, `--format=table`
- Separate commands: `normalize view-json`, `normalize view-table`

**Why**: JSON is the universal interchange format. jq is powerful enough for any transformation. Adding more formats means more code to maintain.

**Trade-off**: jq syntax has learning curve. Mitigation: common patterns documented, `--jq` implies `--json` for convenience.

---

## Adding New Decisions

When making a unification decision, document:
1. **What was unified**: The N things → 1 approach
2. **Alternatives rejected**: What we didn't do and why
3. **Why this works**: The composability/simplicity gain
4. **Trade-off**: What we gave up, how we mitigate it
