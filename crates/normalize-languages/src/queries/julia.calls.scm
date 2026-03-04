; Julia calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func(args)
(call_expression
  (identifier) @call)

; Method call: obj.method(args) — field_expression as callee
(call_expression
  (field_expression
    (_) @call.qualifier
    (identifier) @call))

; Broadcast call: func.(args) — vectorized application
(broadcast_call_expression
  (identifier) @call)

; Broadcast method call: obj.method.(args)
(broadcast_call_expression
  (field_expression
    (_) @call.qualifier
    (identifier) @call))
