; Clojure imports query
; @import       — the entire require/import form (for line number)
; @import.path  — the namespace being required
; @import.alias — namespace alias (:as alias)

; (require '[namespace.core :as nc])
(list_lit
  (sym_lit) @_f (#eq? @_f "require")
  (vec_lit
    (sym_lit) @import.path)) @import

; (ns my.ns (:require [other.ns :as o]))
(list_lit
  (sym_lit) @_f (#eq? @_f "ns")
  (list_lit
    (kwd_lit) @_req (#eq? @_req ":require")
    (vec_lit
      (sym_lit) @import.path))) @import
