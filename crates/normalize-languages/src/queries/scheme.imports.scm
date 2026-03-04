; Scheme imports query
; @import       — the entire import/require form (for line number)
; @import.path  — the library being imported

; (import (library name))
(list
  (symbol) @_f (#eq? @_f "import")
  (_) @import.path) @import

; (require 'library)
(list
  (symbol) @_f (#eq? @_f "require")
  (_) @import.path) @import
