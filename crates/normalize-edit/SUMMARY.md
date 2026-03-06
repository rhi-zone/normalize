# normalize-edit

Structural code editing: find, insert, delete, and replace symbols and containers using tree-sitter ASTs.

Key types: `Editor`, `SymbolLocation`. Key methods on `Editor`: `find_symbol` (locates a symbol by name using `normalize-facts` extraction), `delete_symbol`, `replace_symbol`, `insert_before`, `insert_after`, `prepend_to_file`, `append_to_file`, `find_container_body` (locates a class/impl body via `tags.scm`), `prepend_to_container`, `append_to_container`, and `rename_identifier_in_line`. Also exports `line_to_byte` and `ContainerBody` (re-exported from `normalize-languages`).
