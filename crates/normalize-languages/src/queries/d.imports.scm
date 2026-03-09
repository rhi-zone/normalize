; D imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module being imported

; import std.stdio;
(import_declaration
  (import_list
    (import
      (module_fully_qualified_name) @import.path))) @import

; import std.math : sqrt;  (bindings form)
(import_declaration
  (import_list
    (import_bindings
      (import
        (module_fully_qualified_name) @import.path)))) @import
