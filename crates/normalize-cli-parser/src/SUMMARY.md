# normalize-cli-parser/src

Source modules for the CLI parser crate.

- `lib.rs` — public API: `CliSpec`, `CliOption`, `CliCommand` structs; `parse_help()`, `parse_help_with_format()` functions; re-exports from `formats`.
- `formats/mod.rs` — `CliFormat` trait (`name`, `detect`, `parse`), `FormatRegistry` (isolated instance), and global registry functions (`register`, `get_format`, `detect_format`, `list_formats`). Contains shared helpers `is_section_header()`, `parse_option_command_sections()`, `parse_command_from_trimmed_line()` used across format parsers.
- `formats/clap.rs` — `ClapFormat`; parses `name version\n\nUsage:` header style.
- `formats/argparse.rs` — `ArgparseFormat`; parses Python argparse `usage: prog` style.
- `formats/click.rs` — `ClickFormat`; parses Python click `Usage: prog [OPTIONS]` style.
- `formats/cobra.rs` — `CobraFormat`; parses Go cobra `Available Commands:` style.
- `formats/commander.rs` — `CommanderFormat`; parses Node.js commander `Usage: prog [options] [command]` style.
- `formats/yargs.rs` — `YargsFormat`; parses Node.js yargs `<prog> <cmd>` style with `[type]`/`[default:]` annotations.
