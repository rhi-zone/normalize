# fixtures/commander

Captured help output and example program for Node.js's `commander.js` framework.

Contains `example.js`, `package.json`, `package-lock.json`, `node_modules/`, and `example.help` (captured `--help` output). Used by `tests/commander_fixtures.rs` to verify that `CommanderFormat` correctly parses commander-style help text, which uses `Usage:` lines with `[options] [command]` and an `Options:` / `Commands:` section layout.
