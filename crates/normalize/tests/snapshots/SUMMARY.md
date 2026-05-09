# tests/snapshots

Insta snapshot files for CLI help output tests. Each `.snap` file captures the `--help` text for one normalize subcommand (e.g. `cli_snapshots__help_view.snap`). Covers the root command and all major subcommands including analyze sub-subcommands, daemon, edit (including add-parameter, introduce-variable, inline-variable, move), view, sessions, rules (including setup), serve, ratchet, budget, context, trend, package, rank, grammars, and sync. Run `cargo insta accept` after intentional CLI changes.

The `cli_snapshots__help_context.snap` snapshot now includes comprehensive inline reference documentation in the `after_help` section: frontmatter format, `--match` syntax, `--stdin`/`--prefix` usage, `--file` structured file loading, and common invocation examples.

