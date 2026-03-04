; R calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for namespace-qualified calls
;
; In R's tree-sitter grammar, function calls are `call` nodes.
; Namespace-qualified calls use `::` or `:::` operators (namespace_get).

; Simple call: func(args)
(call
  function: (identifier) @call)

; Namespace-qualified call: pkg::func(args) or pkg:::func(args)
(call
  function: (namespace_get
    namespace: (_) @call.qualifier
    function: (identifier) @call))
