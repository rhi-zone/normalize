; C++ imports query
; @import       — the entire #include or import directive (for line number)
; @import.path  — the header file path (quotes/angle-brackets stripped by Rust)

; #include "local_header.h"
(preproc_include
  path: (string_literal) @import.path) @import

; #include <system_header.h>
(preproc_include
  path: (system_lib_string) @import.path) @import
