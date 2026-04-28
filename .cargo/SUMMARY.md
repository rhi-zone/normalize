# .cargo

Cargo configuration for the workspace.

- `config.toml` — declares the `xtask` alias (`cargo xtask = run -p xtask --`) and tunes dev/release profiles to reduce `target/` size (line-tables-only debug info, optimized deps, stripped release binaries). Includes a commented-out template for opting into the mold linker locally.
