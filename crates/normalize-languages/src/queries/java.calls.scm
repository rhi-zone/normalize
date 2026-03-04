; Java calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Method invocation: obj.method() or method()
(method_invocation
  object: (_) @call.qualifier
  name: (identifier) @call)

(method_invocation
  name: (identifier) @call)
