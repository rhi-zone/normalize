; Scala calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Method call: obj.method()
(call_expression
  function: (field_expression
    value: (_) @call.qualifier
    field: (identifier) @call))

; Generic/type-parameterized call: func[T]()
(call_expression
  function: (generic_function
    function: (identifier) @call))

; Qualified generic call: obj.method[T]()
(call_expression
  function: (generic_function
    function: (field_expression
      value: (_) @call.qualifier
      field: (identifier) @call)))
