; HLSL calls query
; HLSL is C-like; node types mirror tree-sitter-c.
; @call — function being called
; @call.qualifier — receiver for member/pointer calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Member/method call: obj.Method()
(call_expression
  function: (field_expression
    argument: (_) @call.qualifier
    field: (field_identifier) @call))
