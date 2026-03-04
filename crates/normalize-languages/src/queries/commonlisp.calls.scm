; Common Lisp calls query
; @call — call expression nodes
; @call.qualifier — not applicable (Lisp has no method dispatch at this level)
;
; Common Lisp is a Lisp: function application is `(f args...)` where f is the
; first element of a list. In the tree-sitter grammar, this is a `list_lit`
; with a leading `sym_lit` (symbol) as the function position.
;
; We match the outer list_lit as the call context and the first sym_lit as
; @call (the callee name). Qualified symbols like `pkg:func` or `pkg::func`
; appear as `sym_lit` nodes and are captured as-is.

; Function call: (func args...)
(list_lit
  .
  (sym_lit) @call)

; Keyword-named call: (:method args...) — less common but valid
(list_lit
  .
  (kwd_lit) @call)
