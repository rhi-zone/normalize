; Swift imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module identifier

; import Module
; import class Module.Type  (with import kind keyword — kind is modifiers child)
(import_declaration
  (identifier) @import.path) @import
