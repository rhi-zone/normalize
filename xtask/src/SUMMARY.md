# xtask Source

Single-file Rust source for the xtask build automation binary. `main.rs` dispatches on the subcommand argument and implements `build-grammars`: it locates arborium grammar crates in the Cargo registry source cache, compiles each grammar's C source into a shared library via `cc`, and copies workspace-level `locals.scm` query files into the output directory. Supports `--out <dir>` and `--force` flags.
