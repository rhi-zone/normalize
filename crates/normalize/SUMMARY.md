# normalize (main binary crate)

The primary CLI binary crate that wires together all normalize sub-crates into the `normalize` command. It depends on ~30 domain crates (`normalize-facts`, `normalize-languages`, `normalize-edit`, `normalize-session-analysis`, etc.) and exposes them through a unified service layer. Three embedded CLI tools are included as optional features: `rg` (ripgrep), `jq` (jaq), and `ast-grep`/`sg`, each invocable via subcommand or via symlink to the binary.
