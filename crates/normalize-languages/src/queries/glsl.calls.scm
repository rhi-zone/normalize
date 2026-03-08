; GLSL calls query
; GLSL is C-like; node types mirror tree-sitter-c.
; @call — function being called
; @call.qualifier — receiver for member/pointer calls

; Simple call: func()
(call_expression
  function: (identifier) @call)

; Field/member call: obj.method()
(call_expression
  function: (field_expression
    argument: (_) @call.qualifier
    field: (field_identifier) @call))
