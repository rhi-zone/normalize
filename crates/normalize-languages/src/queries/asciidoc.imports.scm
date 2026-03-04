; AsciiDoc imports query
; @import       — the entire include:: macro (for line number)
; @import.path  — the path argument of the include macro

; include::path/to/file.adoc[]
(block_macro
  (block_macro_name) @_name
  (#eq? @_name "include")
  (block_macro_attr) @import.path) @import
