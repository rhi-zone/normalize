; Zsh calls query
; @call — command being executed (function or external program)
; @call.qualifier — not applicable (no method receiver concept in shell)
;
; In Zsh (like Bash), every command execution is effectively a function call.
; The tree-sitter grammar represents commands as `command` nodes with a `name`
; field containing a `command_name` node (the program or shell function name).

; Command execution: cmd args...
(command
  name: (command_name) @call)
