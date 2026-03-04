; Java imports query
; @import       — the entire import statement (for line number)
; @import.path  — the fully-qualified class or package path
; @import.glob  — wildcard marker (presence means is_wildcard=true)

; import pkg.Class;
(import_declaration
  (scoped_identifier) @import.path) @import

; import pkg.*;  (wildcard)
(import_declaration
  (asterisk) @import.glob
  (scoped_identifier) @import.path) @import

; import static pkg.Class.method;
(import_declaration
  "static"
  (scoped_identifier) @import.path) @import

; import static pkg.Class.*;
(import_declaration
  "static"
  (asterisk) @import.glob
  (scoped_identifier) @import.path) @import
