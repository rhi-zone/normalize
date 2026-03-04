; Clojure calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for namespace-qualified calls
;
; Clojure is a Lisp: function application is `(f args...)` where f is
; the first element of a list. In the tree-sitter grammar, this is a
; `list_lit` with a leading `sym_lit` (symbol) as the function position.
;
; We match the outer list_lit as @call and the first sym_lit as @call (name).
; Namespace-qualified symbols like `ns/func` are `sym_lit` nodes containing
; a `/`.

; Function call: (func args...)
; Capture the list as @call and the leading symbol name as @call (identifier)
(list_lit
  .
  (sym_lit) @call)

; Namespace-qualified call: (ns/func args...)
; The sym_lit itself serves as the call name; we treat the namespace portion
; as qualifier by convention (sym_lit text contains ns/func)
