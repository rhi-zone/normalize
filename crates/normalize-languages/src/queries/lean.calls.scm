; Lean 4 calls query
; @call — function application nodes
; @call.qualifier — not applicable (Lean uses dot notation differently)
;
; Lean 4 uses juxtaposition for function application.
; The `apply` node represents function application and has a `name` field
; for the applied function.

; Function application: f x y
(apply
  name: (identifier) @call)
