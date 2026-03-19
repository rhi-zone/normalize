# tests

Integration tests for the normalize CLI binary. `cli_snapshots.rs` tests every subcommand's `--help` output using `assert_cmd` and `insta` snapshot testing, ensuring CLI breaking changes are caught during review. Accept `.snap.new` files to approve intentional help text changes. The `snapshots/` directory holds `.snap` files, one per tested help screen. Updated 2026-03-20 (updated sessions snapshots for --mode flag and subagents subcommand).
