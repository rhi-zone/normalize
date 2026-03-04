; Go calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Method/package call: obj.method() or pkg.Func()
(call_expression
  function: (selector_expression
    operand: (_) @call.qualifier
    field: (field_identifier) @call))
