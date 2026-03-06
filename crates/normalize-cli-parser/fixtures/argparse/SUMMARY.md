# fixtures/argparse

Captured help output and example program for Python's `argparse` framework.

Contains `example.py` (a minimal argparse CLI with subcommands, flags, and a default value) and `example.help` (the captured `--help` output). Used by `tests/argparse_fixtures.rs` to verify that `ArgparseFormat` correctly parses argparse-style help text including subcommand listing, short/long options, value placeholders, and default values.
