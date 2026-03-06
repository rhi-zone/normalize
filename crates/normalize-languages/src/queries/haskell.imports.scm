; Haskell imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module name (first module_id in module path)
; @import.name  — a single imported name
; @import.alias — module alias (as M)

; import Data.Map / import qualified Data.Map
; Matches any import with a module
(import
  module: (module
    (module_id) @import.path)) @import

; import Data.Map as M / import qualified Data.Map as M
; Matches imports with an alias
(import
  module: (module
    (module_id) @import.path)
  alias: (module
    (module_id) @import.alias)) @import
