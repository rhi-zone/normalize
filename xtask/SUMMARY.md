# xtask

Cargo xtask crate for build automation tasks not expressible as standard `cargo` commands. Marked `publish = false` — this is a development tool only, not published to crates.io.

## Subcommands

- `cargo xtask build-grammars [--out <dir>] [--force] [--target <triple>] [--cc <compiler>]` — compile tree-sitter grammar crates from the Cargo registry into shared libraries (`.so`/`.dylib`/`.dll`) and copy workspace `locals.scm` query files alongside them. `--target` cross-compiles for a non-host triple (used by the release workflow to produce musl-linked grammars for the musl tarball); `--cc` overrides the C compiler.
- `cargo xtask bump-version <new-version> [--dry-run]` — update `version = "..."` in all `normalize-*` `[package]` sections (including `[workspace.package]`) and all `normalize-*` dependency constraints across every `Cargo.toml` in the workspace. Validates semver, prints a per-file summary, and runs `cargo generate-lockfile` after writing.
