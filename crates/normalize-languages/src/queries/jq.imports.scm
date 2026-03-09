; jq imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module path string
; @import.alias — the "as $name" identifier

; import "lib/utils" as utils;
(import_
  (string) @import.path) @import

; import "lib/utils" as $utils;  (with variable alias)
(import_
  (string) @import.path
  (variable) @import.alias) @import
