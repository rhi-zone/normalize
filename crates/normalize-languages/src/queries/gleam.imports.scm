; Gleam imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module path being imported
; @import.name  — a single unqualified import name
; @import.alias — module alias

; import module/path
(import
  (module) @import.path) @import

; import module/path as alias
(import
  (module) @import.path
  (import_alias
    (identifier) @import.alias)) @import

; import module/path.{Type, function}
(import
  (module) @import.path
  (unqualified_imports
    (unqualified_import
      (identifier) @import.name))) @import
