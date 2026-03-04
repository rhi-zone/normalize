; OCaml imports query
; @import       — the entire open statement (for line number)
; @import.path  — the module path being opened

; open Module
; open Module.Sub
(open_module
  (_) @import.path) @import
