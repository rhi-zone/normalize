; Dart calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple function call: func()
(invocation_expression
  function: (identifier) @call)

; Method call: obj.method()
(invocation_expression
  function: (selector_expression
    operand: (_) @call.qualifier
    (identifier) @call))
