; Prolog calls query
; @call — predicate/functor invocation (goal)
; @call.qualifier — not applicable
;
; In Prolog, goals are predicate calls. The tree-sitter grammar represents
; compound terms (functor + argument list) as `functional_notation` nodes
; with a `function` field (an `atom`) and an argument list.

; Predicate call: functor(Args...)
(functional_notation
  function: (atom) @call)
