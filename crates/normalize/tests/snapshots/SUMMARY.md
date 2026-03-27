# tests/snapshots

Insta snapshot files for CLI help output tests. Each `.snap` file captures the `--help` text for one normalize subcommand (e.g. `cli_snapshots__help_view.snap`). Covers the root command and all major subcommands including analyze sub-subcommands, daemon, edit, view, sessions, rules (including setup), serve, ratchet, budget, and context. Run `cargo insta accept` after intentional CLI changes.

