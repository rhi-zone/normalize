; GLSL imports query
; @import       — the entire #include directive (for line number)
; @import.path  — the header file path (quotes/angle-brackets stripped by Rust)

; #include "common.glsl"
(preproc_include
  path: (string_literal) @import.path) @import

; #include <common.glsl>
(preproc_include
  path: (system_lib_string) @import.path) @import
