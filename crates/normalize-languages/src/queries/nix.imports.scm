; Nix imports query
; @import       — the entire import expression (for line number)
; @import.path  — the path being imported

; import ./path.nix
; import <nixpkgs>
(apply_expression
  function: (variable_expression (identifier) @_f (#eq? @_f "import"))
  argument: (_) @import.path) @import
