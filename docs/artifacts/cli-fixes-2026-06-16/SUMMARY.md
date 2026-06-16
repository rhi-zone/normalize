# docs/artifacts/cli-fixes-2026-06-16

CLI output formatting audit session, 2026-06-16.

## Contents

- `rank-formatting-audit.md` — Full catalogue of formatting inconsistencies across all
  22 `normalize rank` subcommands, with representative output snippets, source
  locations, and a house-style recommendation with prioritized fix list.
- `elf-program-name.md` — Root-cause investigation into `normalize.elf` appearing in
  usage strings. Traces the bug from the installed ELF filename through the musl
  wrapper script to clap's `file_name()` program-name derivation. Fix location:
  `crates/normalize/src/main.rs`.
