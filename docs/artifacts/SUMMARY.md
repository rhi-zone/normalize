# docs/artifacts

Dated artifact directories produced by audit or investigation sessions. Each
subdirectory is named `<topic>-<YYYY-MM-DD>` and contains markdown reports,
findings, or reference snapshots from a specific session.

## Contents

- `cli-fixes-2026-06-16/` — CLI formatting audit for `normalize rank` subcommands,
  cataloguing inconsistencies in output style across all 22 subcommands. Also
  contains `elf-program-name.md`: investigation into `normalize.elf` appearing
  in usage strings.
- `sessions-stats-output-2026-06-20/` — Diagnosis of `normalize sessions stats`
  pretty output being broken: `stats` method missing `pretty`/`compact` params so
  `self.pretty` is never set and `format_pretty()` is never called.
