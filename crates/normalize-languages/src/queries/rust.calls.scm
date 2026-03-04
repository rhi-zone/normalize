; Rust calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Scoped call: module::func()
(call_expression
  function: (scoped_identifier
    path: (_) @call.qualifier
    name: (identifier) @call))

; Method call: obj.method()
(call_expression
  function: (field_expression
    value: (_) @call.qualifier
    field: (field_identifier) @call))
