; Perl calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple function call: func(args) or func args
(function_call_expression
  function: (function) @call)

; Method call: $obj->method(args) or Class->method(args)
(method_call_expression
  invocant: (_) @call.qualifier
  method: (method) @call)
