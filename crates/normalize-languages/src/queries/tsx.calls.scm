; TSX calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Method call: obj.method()
(call_expression
  function: (member_expression
    object: (_) @call.qualifier
    property: (property_identifier) @call))
