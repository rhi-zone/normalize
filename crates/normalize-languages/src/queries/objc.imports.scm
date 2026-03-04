; Objective-C imports query
; @import       — the entire #import or #include directive (for line number)
; @import.path  — the header file path (quotes/angles stripped by Rust)

; #import "Header.h"
; #import <Framework/Header.h>
; #include "file.h"
(preproc_include
  path: (_) @import.path) @import
