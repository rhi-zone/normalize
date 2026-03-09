; Visual Basic imports query
; @import       — the entire Imports statement (for line number)
; @import.path  — the namespace being imported
; @import.alias — alias for the namespace

; Imports System.Collections
; Imports System.Collections.Generic
; Imports Alias = Namespace
(imports_statement
  namespace: (namespace_name) @import.path) @import
