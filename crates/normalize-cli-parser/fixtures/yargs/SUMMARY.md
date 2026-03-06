# fixtures/yargs

Captured help output and example program for Node.js's `yargs` framework.

Contains `example.js`, `package.json`, `package-lock.json`, `node_modules/`, and `example.help` (captured `--help` output). Used by `tests/yargs_fixtures.rs` to verify that `YargsFormat` correctly parses yargs-style help text, which lists commands with `<prog> <cmd>` usage lines and uses yargs's characteristic option formatting with `[type]` and `[default: ...]` annotations.
