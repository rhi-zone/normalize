# xtask

Cargo xtask crate for build automation tasks not expressible as standard `cargo` commands. Marked `publish = false` — this is a development tool only, not published to crates.io.

## Subcommands

- `cargo xtask build-grammars [--out <dir>] [--force]` — compile tree-sitter grammar crates from the Cargo registry into shared libraries (`.so`/`.dylib`) and copy workspace `locals.scm` query files alongside them.
- `cargo xtask bump-version <new-version> [--dry-run]` — update `version = "..."` in all `normalize-*` `[package]` sections (including `[workspace.package]`) and all `normalize-*` dependency constraints across every `Cargo.toml` in the workspace. Validates semver, prints a per-file summary, and runs `cargo generate-lockfile` after writing.
