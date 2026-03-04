; Haskell imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module name
; @import.name  — a single imported name
; @import.alias — module alias (as M)
; @import.glob  — wildcard marker (not commonly used in Haskell)

; import Data.Map
(import
  (module_id) @import.path) @import

; import qualified Data.Map as M
(import
  (qualified)
  (module_id) @import.path
  (as)
  (module_id) @import.alias) @import

; import Data.Map (lookup, insert)
(import
  (module_id) @import.path
  (import_list
    (import_name) @import.name)) @import

; import qualified Data.Map (lookup)
(import
  (qualified)
  (module_id) @import.path
  (import_list
    (import_name) @import.name)) @import
