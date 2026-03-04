; Groovy calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Regular function/method call: func(args)
(function_call
  (identifier) @call)

; Juxtaposition call: method arg (Groovy allows no-parens calls)
(juxt_function_call
  (identifier) @call)

; Method call with receiver: obj.method(args)
(function_call
  (dotted_identifier
    (_) @call.qualifier
    (identifier) @call))
