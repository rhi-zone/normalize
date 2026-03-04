; PowerShell imports query
; @import       — the entire import command (for line number)
; @import.path  — the module being imported

; Import-Module ModuleName
; Import-Module "path/to/module"
(command
  name: (command_name) @_cmd (#match? @_cmd "(?i)^import-module$")
  argument: (_) @import.path) @import

; . ./script.ps1 (dot-sourcing)
(command
  name: (command_name) @_dot (#eq? @_dot ".")
  argument: (_) @import.path) @import
