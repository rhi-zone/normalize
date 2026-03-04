; Julia imports query
; @import       — the entire import/using statement (for line number)
; @import.path  — the module being imported
; @import.name  — a single selected name (using Pkg: foo, bar)

; import Pkg
; import Pkg.SubModule
(import_statement
  (_) @import.path) @import

; using Pkg
; using Pkg.SubModule
(using_statement
  (_) @import.path) @import
