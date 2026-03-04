; Emacs Lisp imports query
; @import       — the entire require/load form (for line number)
; @import.path  — the feature/file being required

; (require 'feature)
(list
  (symbol) @_f (#eq? @_f "require")
  (_) @import.path) @import

; (load "file.el")
(list
  (symbol) @_f (#eq? @_f "load")
  (_) @import.path) @import

; (require-theme 'theme)
(list
  (symbol) @_f (#eq? @_f "load-theme")
  (_) @import.path) @import
