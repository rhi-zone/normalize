# normalize-cli-parser/fixtures

Captured `--help` output and example programs for each supported CLI framework.

Each subdirectory contains an `example.help` file (the raw help text captured from a real program) and the source program that produced it (`example.py`, `example.rs`, `example.js`, `main.go`). Clap additionally has `example-build.help` and `example-run.help` for subcommand-specific help. These fixtures are used by the per-format integration tests in `tests/`. Contains: `argparse`, `clap`, `click`, `cobra`, `commander`, `yargs`.
