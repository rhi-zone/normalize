; Source: arborium (tree-sitter-nix), corrected and uncommented.
; Arborium intentionally disabled these with a note that tree-sitter's scope model
; doesn't fit Nix's lazy evaluation semantics (order-independent let bindings,
; recursive attribute sets). Results may be imprecise for mutual recursion but
; useful for basic function parameter and let binding tracking.
;
; Corrections from arborium:
; - let_expression uses (binding_set (binding attrpath: (attrpath (identifier))))
;   not bind: (binding attrpath: (attrpath . (attr_identifier)))
; - rec_attrset_expression similarly corrected

; Attrset-destructuring function: { x, y }: body
(function_expression
  universal: (identifier)? @local.definition
  formals: (formals
    (formal name: (identifier) @local.definition))
  ) @local.scope

; rec attrset: rec { x = 1; ... }
(rec_attrset_expression
  (binding_set
    binding: (binding
      attrpath: (attrpath
        (identifier) @local.definition)))
) @local.scope

; let...in: let x = 1; in ...
(let_expression
  (binding_set
    binding: (binding
      attrpath: (attrpath
        (identifier) @local.definition)))
) @local.scope

; References
(identifier) @local.reference
