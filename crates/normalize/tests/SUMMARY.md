# tests

Integration tests for the normalize CLI binary. `cli_snapshots.rs` tests every subcommand's `--help` output using `assert_cmd` and `insta` snapshot testing — run `cargo insta accept` to approve intentional flag or tagline changes. The `snapshots/` directory holds `.snap` files, one per tested help screen. `daemon_push.rs` spawns an isolated `normalize daemon run` subprocess (via `NORMALIZE_DAEMON_CONFIG_DIR` for socket/lock isolation) and exercises the JSON and binary subscribe push channels end-to-end — confirms `FileChanged` events arrive after real file edits via both wire formats. Includes one `#[ignore]`d regression test (`json_subscribe_delivers_index_refreshed_event`) that documents the `needs_refresh()` 60s gate suppressing daemon refreshes; see `TODO.md`.

