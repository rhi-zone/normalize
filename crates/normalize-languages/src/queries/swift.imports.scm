; Swift imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module identifier

; import Module
(import_declaration
  (identifier) @import.path) @import

; import class Module.Type  (with import kind keyword)
(import_declaration
  (scoped_identifier) @import.path) @import
