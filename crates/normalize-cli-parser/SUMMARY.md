# normalize-cli-parser

Parses `--help` output from real CLI programs across multiple frameworks, extracting structured command/option metadata.

Key types: `CliSpec` (parsed result with name, version, description, options, subcommands), `CliOption` (short/long flags, value placeholder, default, env var), `CliCommand` (name, description, aliases, nested subcommands). Key functions: `parse_help()` (auto-detect format), `parse_help_with_format()` (explicit format). The `CliFormat` trait and `FormatRegistry` support both a global registry (for custom formats registered at startup) and an isolated per-instance registry (for testing). Supported frameworks: clap (Rust), argparse and click (Python), commander and yargs (Node.js), cobra (Go).
