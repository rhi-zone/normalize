; Rust imports query
; @import       — the entire use declaration (for line number)
; @import.path  — the module/crate path
; @import.name  — a single imported name
; @import.alias — alias (as Alias)
; @import.glob  — wildcard marker (presence means is_wildcard=true)

; Simple: use path::Item;
; The scoped_identifier's path is the module, name is the item.
(use_declaration
  argument: (scoped_identifier
    path: (_) @import.path
    name: (identifier) @import.name)) @import

; Simple top-level identifier: use foo;
(use_declaration
  argument: (identifier) @import.name) @import

; Aliased: use path::Item as Alias;
(use_declaration
  argument: (use_as_clause
    path: (scoped_identifier
      path: (_) @import.path
      name: (identifier) @import.name)
    alias: (identifier) @import.alias)) @import

; Aliased top-level: use foo as bar;
(use_declaration
  argument: (use_as_clause
    path: (identifier) @import.name
    alias: (identifier) @import.alias)) @import

; Wildcard: use path::*;
(use_declaration
  argument: (scoped_use_list
    path: (_) @import.path
    list: (use_list (use_wildcard) @import.glob))) @import

; Multi-name: use path::{A, B, C};
(use_declaration
  argument: (scoped_use_list
    path: (_) @import.path
    list: (use_list
      (identifier) @import.name))) @import

; Multi-name aliased: use path::{A as X};
(use_declaration
  argument: (scoped_use_list
    path: (_) @import.path
    list: (use_list
      (use_as_clause
        path: (identifier) @import.name
        alias: (identifier) @import.alias)))) @import
