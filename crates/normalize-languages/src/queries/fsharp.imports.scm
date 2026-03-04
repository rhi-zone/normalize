; F# imports query
; @import       — the entire open declaration (for line number)
; @import.path  — the namespace/module being opened

; open System.Collections
; open Microsoft.FSharp.Core
(import_decl
  (long_identifier) @import.path) @import
