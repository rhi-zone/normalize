; Perl calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple function call: func(args) or func args
(function_call_expression
  function: (identifier) @call)

; Method call: $obj->method(args) or Class->method(args)
(method_call_expression
  object: (_) @call.qualifier
  method: (identifier) @call)
