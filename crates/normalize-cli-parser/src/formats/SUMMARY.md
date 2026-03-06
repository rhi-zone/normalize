# normalize-cli-parser/src/formats

Format-specific parsers implementing the `CliFormat` trait, plus shared parsing utilities.

`mod.rs` defines `CliFormat` (with `name()`, `detect() -> f64`, `parse() -> Result<CliSpec, String>`), the global `FORMATS` registry (a `RwLock<Vec<&'static dyn CliFormat>>`), and shared helpers used by multiple parsers. Detection works by scoring (0.0–1.0 confidence) and selecting the highest scorer above 0.5. The six format modules are: `argparse.rs`, `clap.rs`, `click.rs`, `cobra.rs`, `commander.rs`, `yargs.rs`.
