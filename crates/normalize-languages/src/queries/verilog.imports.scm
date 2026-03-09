; Verilog/SystemVerilog imports query
; @import       — the entire package import declaration (for line number)
; @import.path  — the package being imported
; @import.name  — a specific item imported from the package
; @import.glob  — wildcard import marker (import pkg::*)

; import my_pkg::*;
(package_import_declaration
  (package_import_item
    (package_identifier) @import.path
    "*" @import.glob)) @import

; import my_pkg::my_type;
(package_import_declaration
  (package_import_item
    (package_identifier) @import.path
    (simple_identifier) @import.name)) @import
