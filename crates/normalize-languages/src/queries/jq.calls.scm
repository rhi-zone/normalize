; jq calls query
; @call — the name of the function/builtin being called
; @call.qualifier — not applicable for jq (no method receiver concept)
;
; In jq, function calls are modeled as a `binding` node with a `function` field
; containing a `funcname` child, optionally followed by `args` in parentheses.
; Examples: `map(.)`, `select(.x > 0)`, `recurse`, `split(",")`

; Function call with arguments: func(args)
(binding
  function: (funcname) @call
  (args))

; Function call without arguments (bare name): func
(binding
  function: (funcname) @call .)
