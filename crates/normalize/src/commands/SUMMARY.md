# src/commands

CLI command implementations, one module per top-level subcommand. Each module contains the computation logic, report structs, and `OutputFormatter` implementations for its command; the service layer in `src/service/` wires these into the `#[cli]`-generated CLI. Top-level modules: `analyze`, `context`, `daemon`, `edit`, `facts`, `find_references`, `generate`, `grammars`, `history`, `init`, `package`, `rules`, `sessions`, `text_search`, `tools`, `translate`, `update`, `view`. Shared helper `build_filter()` constructs `Filter` values from `--exclude`/`--only` flags.
