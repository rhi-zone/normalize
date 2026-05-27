# data

Static metadata tables for `normalize-language-meta`. Currently:

- `languages.toml` — per-language capability records (imports, callable
  symbols, paradigm tags, etc.) loaded by `data.rs` into the
  `Capabilities` registry. Source of truth for what each language
  supports; `capabilities_for(name)` reads from here.
