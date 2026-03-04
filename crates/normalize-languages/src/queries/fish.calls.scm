; Fish shell calls query
; @call — command being executed
; @call.qualifier — not applicable
;
; Fish represents command invocations as `command` nodes with a `name` field.

; Command invocation: some_command arg1 arg2
(command
  name: (word) @call)
