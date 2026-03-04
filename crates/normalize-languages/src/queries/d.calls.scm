; D calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; D uses `call_expression` for function/method calls.

; Simple call: func(args)
(call_expression
  function: (identifier) @call)

; Method call: obj.method(args)
(call_expression
  function: (dot_expression
    (_) @call.qualifier
    (identifier) @call))
