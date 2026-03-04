; MATLAB calls query
; @call — function call expression
; @call.qualifier — not applicable (MATLAB method calls go through field_expression)
;
; MATLAB function calls are represented as `function_call` nodes with a `name`
; field containing an `identifier`. Command-syntax calls (`disp x`) appear as
; `command` nodes with a `command_name` child.

; Standard function call: func(args...)
(function_call
  name: (identifier) @call)

; Command syntax: command arg (e.g. `disp x`, `clear all`)
(command
  (command_name) @call)
