; Zig imports query
; @import       — the entire @import call (for line number)
; @import.path  — the module path string (quotes stripped by Rust)

; @import("std") or @import("./file.zig")
(builtin_call_expression
  function: (BUILTINIDENTIFIER) @_f (#eq? @_f "@import")
  arguments: (string_literal) @import.path) @import
