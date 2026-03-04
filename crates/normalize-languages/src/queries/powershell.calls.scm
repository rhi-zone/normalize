; PowerShell calls query
; @call — command invocation nodes
; @call.qualifier — not applicable
;
; PowerShell represents command calls as `command` nodes with a
; `command_name` field.

; Command/function invocation: Get-Item path
(command
  command_name: (command_name) @call)

; Invocation via & or . operator: & $func args
(invokation_expression) @call
