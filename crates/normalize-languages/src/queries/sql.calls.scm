; SQL calls query
; @call — function invocation
; @call.qualifier — not applicable
;
; SQL function calls are represented as `invocation` nodes. An `invocation`
; contains an `object_reference` (the function name, which has a `name` field
; with an `identifier`) followed by a parenthesized argument list.

; Function call: func(args...) — e.g. COUNT(*), COALESCE(a, b), NOW()
(invocation
  (object_reference
    name: (identifier) @call))
