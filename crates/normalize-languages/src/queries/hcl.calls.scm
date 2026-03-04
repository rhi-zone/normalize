; HCL calls query
; @call — function call expression
; @call.qualifier — not applicable (HCL functions are not method calls)
;
; HCL function calls are of the form `func(args...)`. The tree-sitter grammar
; represents these as `function_call` nodes with a leading `identifier` child
; (the function name) and a `function_arguments` child (the argument list).

; Function call: func(args...)
(function_call
  (identifier) @call)
