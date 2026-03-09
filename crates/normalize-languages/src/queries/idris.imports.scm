; Idris imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module being imported

; import Data.List
; import public Data.List
(import
  module: (qualified_caname) @import.path) @import

(import
  module: (caname) @import.path) @import
