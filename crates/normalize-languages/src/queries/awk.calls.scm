; AWK calls query
; @call — function call expression nodes
; @call.qualifier — namespace for namespace-qualified calls
;
; AWK has two call forms:
;   - func_call: named function call with a `name` field
;   - indirect_func_call: indirect call via a variable

; Direct function call: func(args)
(func_call
  name: (identifier) @call)

; Namespace-qualified call: ns::func(args)
(func_call
  name: (ns_qualified_name) @call)
