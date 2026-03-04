; Agda imports query
; @import       — the entire import/open statement (for line number)
; @import.path  — module path (whole statement text)
; @import.glob  — open import marker (wildcard = opens entire namespace)

; import Data.List
(import) @import.path @import

; open import Data.List  (open = wildcard/glob)
(open) @import.path @import.glob @import
