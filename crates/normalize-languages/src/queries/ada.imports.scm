; Ada imports query
; @import       — the entire with/use clause (for line number)
; @import.path  — the package path (whole clause text; Rust strips "with"/"use" keywords)

; with Ada.Text_IO;
(with_clause) @import.path @import

; use Ada.Text_IO;
(use_clause) @import.path @import
