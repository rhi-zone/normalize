; Ruby calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple method call (no receiver): func or func(args)
(call
  method: (identifier) @call)

; Method call with receiver: obj.method or obj.method(args)
(call
  receiver: (_) @call.qualifier
  method: (identifier) @call)
