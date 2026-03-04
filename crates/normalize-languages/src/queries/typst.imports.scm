; Typst imports query
; @import       — the entire #import statement (for line number)
; @import.path  — the module path
; @import.name  — a single imported name
; @import.glob  — wildcard import marker

; #import "lib.typ"
(import
  (string) @import.path) @import

; #import "lib.typ": func1, func2
(import
  (string) @import.path
  (import_items
    (ident) @import.name)) @import

; #import "lib.typ": *
(import
  (string) @import.path
  (wildcard) @import.glob) @import
