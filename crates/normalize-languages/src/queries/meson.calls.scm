; Meson calls query
; @call — function/method call nodes
; @call.qualifier — object receiver for method calls
;
; Meson represents standalone function calls as `normal_command` nodes with a
; `command` field. Method calls on objects appear as `expression_statement`
; nodes with `object` and `function` fields.

; Standalone function call: func_name(args)
(normal_command
  command: (identifier) @call)

; Method call on object: obj.method(args)
(expression_statement
  object: (identifier) @call.qualifier
  function: (normal_command
    command: (identifier) @call))
