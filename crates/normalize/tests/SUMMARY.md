# tests

Integration tests for the normalize CLI binary. `cli_snapshots.rs` tests every subcommand's `--help` output using `assert_cmd` and `insta` snapshot testing, ensuring CLI breaking changes are caught during review. Run `cargo insta review` to approve intentional help text changes. The `snapshots/` directory holds 83 `.snap` files, one per tested help screen.
