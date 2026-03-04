; Gleam calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for module-qualified calls

; Simple call: func(args)
(function_call
  function: (identifier) @call)

; Module-qualified call: module.func(args)
(function_call
  function: (field_access
    record: (_) @call.qualifier
    label: (label) @call))
