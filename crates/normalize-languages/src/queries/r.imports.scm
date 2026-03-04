; R imports query
; @import       — the entire library/require call (for line number)
; @import.path  — the package name being imported

; library(pkg) or library("pkg")
(call
  function: (identifier) @_f (#eq? @_f "library")
  arguments: (arguments
    (_) @import.path)) @import

; require(pkg)
(call
  function: (identifier) @_f (#eq? @_f "require")
  arguments: (arguments
    (_) @import.path)) @import
