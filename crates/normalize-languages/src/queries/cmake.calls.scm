; CMake calls query
; @call — call expression nodes
; @call.qualifier — not applicable
;
; CMake represents function/macro calls as `normal_command` nodes.
; The first child is an `identifier` with the command name.

; Normal command call: some_command(args)
(normal_command
  (identifier) @call)
