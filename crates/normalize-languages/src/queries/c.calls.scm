; C calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Field/pointer member call: obj.method() or ptr->method()
(call_expression
  function: (field_expression
    argument: (_) @call.qualifier
    field: (field_identifier) @call))
