; D imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module being imported
; @import.name  — a single selective import name

; import std.stdio;
(import_declaration
  (import_list
    (import_binding
      (identifier) @import.path))) @import

; import std.stdio : writeln, writefln;
(import_declaration
  (import_list
    (import_binding
      (identifier) @import.path
      (import_bind_list
        (import_bind
          (identifier) @import.name))))) @import
