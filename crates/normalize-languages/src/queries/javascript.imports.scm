; JavaScript imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module path string
; @import.name  — a single imported name
; @import.alias — alias for @import.name or @import.path
; @import.glob  — wildcard marker (presence means is_wildcard=true)

; import 'module' (side-effect only)
(import_statement
  source: (string
    (string_fragment) @import.path)) @import

; import defaultExport from 'module'
(import_statement
  (import_clause
    (identifier) @import.name)
  source: (string
    (string_fragment) @import.path)) @import

; import { name } from 'module'
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @import.name)))
  source: (string
    (string_fragment) @import.path)) @import

; import { name as alias } from 'module'
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @import.name
        alias: (identifier) @import.alias)))
  source: (string
    (string_fragment) @import.path)) @import

; import * as ns from 'module'
(import_statement
  (import_clause
    (namespace_import
      (identifier) @import.alias))
  source: (string
    (string_fragment) @import.path)) @import
