; Ninja imports query
; @import       — the entire include/subninja statement (for line number)
; @import.path  — the file path being included

; include rules.ninja
(include
  (path) @import.path) @import

; subninja subdir/build.ninja
(subninja
  (path) @import.path) @import
