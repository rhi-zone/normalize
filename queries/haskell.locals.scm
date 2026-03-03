; Source: arborium (tree-sitter-haskell), extended.
; Arborium used (pattern/variable) and (expression/variable) — Neovim-specific
; field-path syntax not supported by the standard tree-sitter query API.
; Replaced with confirmed node types from the grammar.
;
; Note: top-level function definitions (with patterns) are "function" nodes;
; simple bindings (name = expr, no patterns) are "bind" nodes.

; Scopes
[
  (function)
  (let_in)
] @local.scope

; Definitions

; Function name: add x y = ...
(function
  name: (variable) @local.definition)

; Simple binding name: x = ... (used in let/where/top-level no-pattern binds)
(bind
  name: (variable) @local.definition)

; Type signature (forward declaration)
(signature
  name: (variable) @local.definition)

; Function parameters (variables in patterns list)
(function
  patterns: (patterns
    (variable) @local.definition))

; References
(variable) @local.reference
