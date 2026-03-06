# normalize-cli-parser/tests

Integration tests for each CLI format parser, one file per framework.

Each test file (`argparse_fixtures.rs`, `clap_fixtures.rs`, `click_fixtures.rs`, `cobra_fixtures.rs`, `commander_fixtures.rs`, `yargs_fixtures.rs`) includes the corresponding `fixtures/<format>/example.help` via `include_str!` and asserts that `parse_help()` auto-detects the correct format, and that `parse_help_with_format()` extracts the expected name, description, subcommands, options, value placeholders, defaults, and aliases. Tests use `insta` for snapshot assertions where applicable.
