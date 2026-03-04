; Common Lisp imports query
; @import       — the entire require/use-package form (for line number)
; @import.path  — the package being imported

; (require 'package) or (require "package")
(list_lit
  (sym_lit) @_f (#eq? @_f "require")
  (_) @import.path) @import

; (use-package :package)
(list_lit
  (sym_lit) @_f (#eq? @_f "use-package")
  (_) @import.path) @import

; (ql:quickload :package)
(list_lit
  (sym_lit) @_f (#eq? @_f "ql:quickload")
  (_) @import.path) @import
