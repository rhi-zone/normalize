; Lua calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func() or func(args)
(function_call
  name: (identifier) @call)

; Method call: obj:method() — colon syntax
(function_call
  name: (method_index_expression
    table: (_) @call.qualifier
    method: (identifier) @call))

; Field call: obj.func()
(function_call
  name: (dot_index_expression
    table: (_) @call.qualifier
    field: (identifier) @call))
