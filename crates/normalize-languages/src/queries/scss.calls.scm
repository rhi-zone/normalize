; SCSS calls query
; @call — function call expression nodes
; @call.qualifier — not applicable (SCSS functions are not method calls)
;
; SCSS function calls appear as `call_expression` nodes with a `function_name`
; child (the callee) and an `arguments` child. Examples:
;   rgba(255, 0, 0, 0.5)
;   darken($color, 10%)
;   map-get($map, key)

; Function call: func(args...)
(call_expression
  (function_name) @call)
