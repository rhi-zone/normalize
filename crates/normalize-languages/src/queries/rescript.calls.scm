; ReScript calls query
; @call — call expression
; @call.qualifier — module qualifier for qualified calls
;
; ReScript (formerly BuckleScript/Reason) has ML-style function calls.
; Call expressions have a `function` field that is a `primary_expression`,
; which is either a `value_identifier` (simple call) or a
; `value_identifier_path` (module-qualified call like `List.map`).

; Simple call: func(args...)
(call_expression
  function: (value_identifier) @call)

; Module-qualified call: Module.func(args...)
(call_expression
  function: (value_identifier_path
    (value_identifier) @call))
