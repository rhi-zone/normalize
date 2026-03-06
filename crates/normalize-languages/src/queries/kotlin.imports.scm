; Kotlin imports query
; @import       — the entire import header (for line number)
; @import.path  — the module/class path
; @import.alias — alias after 'as'
; @import.glob  — wildcard marker (presence means is_wildcard=true)

; import pkg.Class
(import_header
  (identifier) @import.path) @import

; import pkg.Class as Alias
(import_header
  (identifier) @import.path
  (import_alias
    (type_identifier) @import.alias)) @import

; import pkg.*
(import_header
  (identifier) @import.path
  (wildcard_import) @import.glob) @import
