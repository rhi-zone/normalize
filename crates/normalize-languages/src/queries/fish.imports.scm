; Fish shell imports query
; @import       — the entire source command (for line number)
; @import.path  — the file path being sourced

; source file.fish
(command
  name: (word) @_cmd
  argument: (_) @import.path
  (#eq? @_cmd "source")) @import
