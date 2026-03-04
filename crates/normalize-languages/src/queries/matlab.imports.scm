; MATLAB imports query
; @import       — the entire import command (for line number)
; @import.path  — the package/class being imported

; import pkg.Class
; import pkg.*
(command
  name: (identifier) @_cmd (#eq? @_cmd "import")
  argument: (_) @import.path) @import
