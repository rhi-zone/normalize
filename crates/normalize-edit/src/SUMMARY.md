# normalize-edit/src

Single-file source for the `normalize-edit` crate.

`lib.rs` implements all editing operations. `find_symbol` uses `normalize_facts::Extractor` to build the symbol tree, then searches recursively. Container body location (`find_container_body_via_tags`) runs the language's `tags.scm` query to find `@definition.class/module/interface` nodes matching by name, then calls the `Language` trait's `container_body` and `analyze_container_body` hooks. `rename_identifier_in_line` does whole-word replacement on a single line using `replace_all_words`.
