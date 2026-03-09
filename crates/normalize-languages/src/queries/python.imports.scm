; Python imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module path being imported
; @import.name  — a single imported name
; @import.alias — alias for @import.name or @import.path
; @import.glob  — wildcard marker (presence means is_wildcard=true)

; import foo
(import_statement
  name: (dotted_name) @import.path) @import

; import foo as bar
(import_statement
  name: (aliased_import
    name: (dotted_name) @import.path
    alias: (identifier) @import.alias)) @import

; from foo import bar
(import_from_statement
  module_name: (_) @import.path
  name: (dotted_name) @import.name) @import

; from foo import bar (single name as dotted_name with one part)
(import_from_statement
  module_name: (_) @import.path
  name: (dotted_name) @import.name) @import

; from foo import bar as baz
(import_from_statement
  module_name: (_) @import.path
  name: (aliased_import
    name: (_) @import.name
    alias: (identifier) @import.alias)) @import

; from foo import *
(import_from_statement
  module_name: (_) @import.path
  (wildcard_import) @import.glob) @import
