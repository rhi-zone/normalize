; C# calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple invocation: Method()
(invocation_expression
  function: (identifier_name) @call)

; Member access invocation: obj.Method()
(invocation_expression
  function: (member_access_expression
    expression: (_) @call.qualifier
    name: (identifier_name) @call))
