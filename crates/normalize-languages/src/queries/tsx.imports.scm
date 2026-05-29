; TSX imports query (identical to TypeScript — TSX shares TS import syntax)
; @import          — the entire import statement (for line number)
; @import.path     — the module path string
; @import.name     — a single imported name
; @import.alias    — alias for @import.name or @import.path
; @import.glob     — wildcard marker (presence means is_wildcard=true)
; @import.reexport — presence means this is an `export ... from` re-export

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

; export * as ns from 'module'  (namespace re-export — star with alias)
(export_statement
  (namespace_export
    "*" @import.glob
    (identifier) @import.alias)
  source: (string
    (string_fragment) @import.path)) @import @import.reexport

; const x = require('module')
(lexical_declaration
  (variable_declarator
    name: (identifier) @import.name
    value: (call_expression
      function: (identifier) @_fn
      arguments: (arguments
        (string
          (string_fragment) @import.path))))
  (#eq? @_fn "require")) @import

; var x = require('module')
(variable_declaration
  (variable_declarator
    name: (identifier) @import.name
    value: (call_expression
      function: (identifier) @_fn
      arguments: (arguments
        (string
          (string_fragment) @import.path))))
  (#eq? @_fn "require")) @import

; const { a, b } = require('module')  (shorthand destructured — one match per name)
(lexical_declaration
  (variable_declarator
    name: (object_pattern
      (shorthand_property_identifier_pattern) @import.name)
    value: (call_expression
      function: (identifier) @_fn
      arguments: (arguments
        (string
          (string_fragment) @import.path))))
  (#eq? @_fn "require")) @import

; var { a, b } = require('module')  (shorthand destructured — one match per name)
(variable_declaration
  (variable_declarator
    name: (object_pattern
      (shorthand_property_identifier_pattern) @import.name)
    value: (call_expression
      function: (identifier) @_fn
      arguments: (arguments
        (string
          (string_fragment) @import.path))))
  (#eq? @_fn "require")) @import

; const { key: alias } = require('module')  (aliased destructured — use the bound name)
(lexical_declaration
  (variable_declarator
    name: (object_pattern
      (pair_pattern
        value: (identifier) @import.name))
    value: (call_expression
      function: (identifier) @_fn
      arguments: (arguments
        (string
          (string_fragment) @import.path))))
  (#eq? @_fn "require")) @import

; var { key: alias } = require('module')  (aliased destructured — use the bound name)
(variable_declaration
  (variable_declarator
    name: (object_pattern
      (pair_pattern
        value: (identifier) @import.name))
    value: (call_expression
      function: (identifier) @_fn
      arguments: (arguments
        (string
          (string_fragment) @import.path))))
  (#eq? @_fn "require")) @import

; require('module')  (side-effect only, no binding)
(expression_statement
  (call_expression
    function: (identifier) @_fn
    arguments: (arguments
      (string
        (string_fragment) @import.path)))
  (#eq? @_fn "require")) @import
