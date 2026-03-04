; Bash imports query
; @import       — the entire source command (for line number)
; @import.path  — the file path being sourced

; source file.sh
(command
  name: (command_name) @_cmd (#eq? @_cmd "source")
  argument: (_) @import.path) @import

; . file.sh (POSIX dot command)
(command
  name: (command_name) @_dot (#eq? @_dot ".")
  argument: (_) @import.path) @import
