; TypeScript imports query
; @import          — the entire import/export-from statement (for line number)
; @import.path     — the module path string
; @import.name     — a single imported name
; @import.alias    — alias for @import.name or @import.path
; @import.glob     — wildcard marker (presence means is_wildcard=true)
; @import.reexport — presence means this is an `export ... from` re-export
;
; For re-exports, the export_statement node is captured as BOTH @import (anchor) and
; @import.reexport (flag), so the query runner sets both stmt_line and is_reexport.

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

; import type { Foo } from 'module'
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @import.name)))
  source: (string
    (string_fragment) @import.path)) @import

; export { name } from 'module'  (re-export)
(export_statement
  (export_clause
    (export_specifier
      name: (identifier) @import.name))
  source: (string
    (string_fragment) @import.path)) @import @import.reexport

; export { name as alias } from 'module'  (re-export with alias)
(export_statement
  (export_clause
    (export_specifier
      name: (identifier) @import.name
      alias: (identifier) @import.alias))
  source: (string
    (string_fragment) @import.path)) @import @import.reexport

; export * from 'module'  (wildcard re-export — bare star, no alias)
(export_statement
  "*" @import.glob
  source: (string
    (string_fragment) @import.path)) @import @import.reexport

; export * as ns from 'module'  (namespace re-export — star with alias, not wildcard)
(export_statement
  (namespace_export
    (identifier) @import.alias)
  source: (string
    (string_fragment) @import.path)) @import @import.reexport
