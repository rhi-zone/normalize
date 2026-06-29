# cli-audit-2026-06-29

Read-only audit of the `normalize` CLI surface, run by five parallel agents on 2026-06-29
and consolidated into a single prioritized triage.

## Contents

- `00-triage.md` — start here. De-duplicated, tier-ranked master catalogue (T1/T2/T3)
  merging the overlapping entries from the five audits below. Marks hard-constraint
  violations, owning repo (normalize vs server-less), and effort. Includes the verdict on
  the `budget --base-ref`->`--diff-ref` rename (keep it; document in CHANGELOG).
- `01-structured-output.md` — `--json`/`--jsonl` coverage across 103 commands.
- `02-flag-naming.md` — flag-name consistency + the server-less 0.6 `#[param(name)]` rename.
- `03-dry-run.md` — `--dry-run` coverage; 22 mutating commands violate the hard constraint.
- `04-errors-exit-codes.md` — exit codes + error channels; exit-0-on-missing-index class.
- `05-command-structure.md` — command-tree shape, duplicates, broken guides.

Actionable items are recorded in the repo `TODO.md` under "CLI audit 2026-06-29 backlog".
