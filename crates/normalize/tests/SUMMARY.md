# tests

Integration tests for the normalize CLI binary. `cli_snapshots.rs` tests every subcommand's `--help` output using `assert_cmd` and `insta` snapshot testing — run `cargo insta accept` to approve intentional flag or tagline changes. The `snapshots/` directory holds `.snap` files, one per tested help screen.

