# src (normalize crate source root)

Root of the normalize library and binary. `main.rs` handles argv[0] dispatch (symlink-as-drop-in for `rg`, `jq`, `sg`) and delegates to the server-less service layer or legacy clap dispatch. `lib.rs` declares all top-level modules. Key infrastructure files at this level: `config.rs` (NormalizeConfig/TOML), `output.rs` (OutputFormatter trait + format dispatch), `rules.rs` (unified rule execution via syntax/fact/SARIF engines), `diagnostic_convert.rs` (Finding/ABI diagnostic → Issue), `index.rs`, `symbols.rs`, `parsers.rs`, `extract.rs`, `skeleton.rs`, `tree.rs`.

`commands/` contains domain logic modules; dead `cmd_*` i32-returning wrappers were eliminated — service methods now call analysis functions directly. `service/` is the primary CLI registration point (server-less `#[cli]` proc macro).
