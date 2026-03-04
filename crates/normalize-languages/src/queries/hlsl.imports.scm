; HLSL imports query
; @import       — the entire #include directive (for line number)
; @import.path  — the header file path

; #include "common.hlsl"
(preproc_include
  path: (string_literal) @import.path) @import

; #include <d3d11.h>
(preproc_include
  path: (system_lib_string) @import.path) @import
