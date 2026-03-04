; Fish shell imports query
; @import       — the entire source command (for line number)
; @import.path  — the file path being sourced

; source file.fish
(command
  name: (command_name) @_cmd (#eq? @_cmd "source")
  argument: (_) @import.path) @import
