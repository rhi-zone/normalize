; Lua imports query
; @import       — the entire require() call (for line number)
; @import.path  — the module path string (quotes stripped by Rust)

; require("module") or require('module')
(function_call
  name: (identifier) @_require (#eq? @_require "require")
  arguments: (arguments
    (string) @import.path)) @import
