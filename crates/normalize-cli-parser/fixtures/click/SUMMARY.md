# fixtures/click

Captured help output and example program for Python's `click` framework.

Contains `example.py` (a minimal click CLI with subcommands and options) and `example.help` (the captured `--help` output). Used by `tests/click_fixtures.rs` to verify that `ClickFormat` correctly parses click-style help text, which uses `Usage: ...` lines and indented option/command listings distinct from argparse's format.
