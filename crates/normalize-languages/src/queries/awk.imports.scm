; AWK (GAWK) imports query
; @import       — the entire directive (for line number)
; @import.path  — the file path being included/loaded

; @include "file.awk"
; @load "extension"
(directive
  (string) @import.path) @import
