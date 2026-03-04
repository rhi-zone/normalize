; Scheme calls query
; @call — function application nodes
; @call.qualifier — not applicable
;
; Scheme (and Lisps generally) use `(proc arg ...)` list syntax for calls.
; The grammar represents everything as `list` nodes. The first element of
; a list is the operator/function being called. A `symbol` as the first
; child is a named function call.

; Function call: (proc arg1 arg2 ...)
; The first child of a list that is a symbol names the called function.
(list
  .
  (symbol) @call)
