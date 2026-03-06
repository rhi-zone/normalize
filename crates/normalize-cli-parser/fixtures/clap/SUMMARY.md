# fixtures/clap

Captured help output and example program for Rust's `clap` framework.

Contains `example.rs` (a minimal clap CLI with subcommands and options), `Cargo.toml`, `example.help` (top-level `--help` output), `example-build.help`, and `example-run.help` (subcommand-specific help). Used by `tests/clap_fixtures.rs` to verify that `ClapFormat` correctly parses clap-style help text, including the distinct clap header format (`name version\ndescription\n\nUsage:`) and subcommand-specific help pages.
