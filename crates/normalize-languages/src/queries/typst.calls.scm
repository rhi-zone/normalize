; Typst calls query
; Typst uses `call` nodes for both definitions (#let f(x) = ...) and call sites (f(x)).
; When run against function bodies the `let` wrapper is absent, so all `call` nodes
; in scope are genuine call sites.
; @call — function being called

; Simple call: func(args)
(call
  item: (ident) @call)
