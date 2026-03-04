; SCSS imports query
; @import       — the entire @import/@use/@forward statement (for line number)
; @import.path  — the file path being imported

; @import "variables";
(import_statement
  (string_value) @import.path) @import

; @use "sass:math";
(use_statement
  (string_value) @import.path) @import

; @forward "mixins";
(forward_statement
  (string_value) @import.path) @import
