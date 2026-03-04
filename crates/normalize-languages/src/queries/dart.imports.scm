; Dart imports query
; @import       — the entire import/export specification (for line number)
; @import.path  — the URI string (quotes stripped by Rust)
; @import.name  — an identifier from a show/hide combinator
; @import.alias — alias after 'as'

; import 'uri';
(import_specification
  uri: (string_literal
    (string_content) @import.path)) @import

; import 'uri' as alias;
(import_specification
  uri: (string_literal
    (string_content) @import.path)
  (as_clause
    (identifier) @import.alias)) @import

; import 'uri' show A, B;  (one match per name)
(import_specification
  uri: (string_literal
    (string_content) @import.path)
  (show_combinator
    (identifier) @import.name)) @import

; import 'uri' hide A, B;  (one match per name)
(import_specification
  uri: (string_literal
    (string_content) @import.path)
  (hide_combinator
    (identifier) @import.name)) @import

; export 'uri';
(library_export
  uri: (string_literal
    (string_content) @import.path)) @import
