; ReScript imports query
; @import       — the entire open statement (for line number)
; @import.path  — the module being opened

; open Module
; open Module.Sub
(open_statement
  (_) @import.path) @import
