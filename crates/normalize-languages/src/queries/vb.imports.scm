; Visual Basic imports query
; @import       — the entire Imports statement (for line number)
; @import.path  — the namespace being imported
; @import.alias — alias for the namespace

; Imports System.Collections
; Imports System.Collections.Generic
(imports_statement
  (imports_member_name) @import.path) @import

; Imports Alias = Namespace
(imports_statement
  (imports_alias_clause
    name: (identifier) @import.alias
    (imports_member_name) @import.path)) @import
