# src/rg/flags/doc

Documentation generators for the embedded ripgrep CLI, vendored from ripgrep 14.1.1. Produces short help (`template.short.help`), long help (`template.long.help`), and a man page (`template.rg.1`) from flag definitions at runtime via `help.rs` and `man.rs`. `version.rs` formats the version string. `render_custom_markup` handles `\tag{...}` substitution in doc templates. These support `normalize rg --help` and `normalize rg --generate man`.
