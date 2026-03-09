; Elm imports query
; @import       — the entire import clause (for line number)
; @import.path  — the module name
; @import.alias — the "as Alias" name
; @import.glob  — exposing (..) wildcard marker
; @import.name  — a single exposed name

; import Html
(import_clause
  moduleName: (upper_case_qid) @import.path) @import

; import Html as H
(import_clause
  moduleName: (upper_case_qid) @import.path
  asClause: (as_clause
    (upper_case_identifier) @import.alias)) @import

; import Html exposing (..)
(import_clause
  moduleName: (upper_case_qid) @import.path
  exposing: (exposing_list
    (double_dot) @import.glob)) @import

; import Html exposing (div, span)
(import_clause
  moduleName: (upper_case_qid) @import.path
  exposing: (exposing_list
    (exposed_value) @import.name)) @import
