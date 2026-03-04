; Nix calls query
; @call — function application expression
; @call.qualifier — not applicable
;
; Nix uses juxtaposition for function application: `f x` (no parentheses).
; The tree-sitter grammar represents this as `apply_expression` with a
; `function` field (the callee) and an `argument` field.
;
; The callee is either a `variable_expression` (simple name) or a
; `select_expression` (attribute path like `builtins.map`).

; Simple application: f arg
(apply_expression
  function: (variable_expression
    (identifier) @call))

; Attribute-path application: builtins.map, lib.lists.map, etc.
(apply_expression
  function: (select_expression
    attrpath: (attrpath
      attr: (identifier) @call .)))
