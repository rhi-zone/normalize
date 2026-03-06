# normalize-path-resolve/src

Single-file source for the `normalize-path-resolve` crate.

`lib.rs` implements the full resolution pipeline. `resolve_from_paths` tries in order: glob patterns, normalized exact match (treating `-`/`.`/`_` as equivalent), filename/stem match, path suffix match, then nucleo fuzzy scoring (top 10). `resolve_unified` walks segments left-to-right against the filesystem to find the file/symbol split boundary, falling back to fuzzy matching. `expand_sigil` parses `@name[sep]suffix` using the provided alias lookup closure. `normalize_separators` converts `::`, `#`, `:` to `/`.
