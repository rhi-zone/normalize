; PHP calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple function call: func(args)
(function_call_expression
  function: (name) @call)

; Variable function call: $func(args)
(function_call_expression
  function: (variable_name
    (name) @call))

; Static method call: Class::method(args)
(scoped_call_expression
  scope: (_) @call.qualifier
  name: (name) @call)

; Instance method call: $obj->method(args)
(member_call_expression
  object: (_) @call.qualifier
  name: (name) @call)

; Nullsafe method call: $obj?->method(args)
(nullsafe_member_call_expression
  object: (_) @call.qualifier
  name: (name) @call)
