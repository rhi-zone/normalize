; jq imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module path string
; @import.alias — the "as $name" identifier

; import "module" as name;
(import
  (import_) @import.path) @import
