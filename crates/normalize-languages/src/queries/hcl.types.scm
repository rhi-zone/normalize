; HCL types query
; @type — type constraint expressions in variable blocks
;
; HCL (Terraform) uses type constraints in `variable` blocks:
;   variable "example" {
;     type = string
;     type = list(string)
;     type = object({ name = string })
;   }
;
; In the tree-sitter grammar, `type = string` is an `attribute` where the
; first child identifier is literally "type" and the second child is an
; expression. We capture the expression as the type reference.

; Type constraint attribute: type = string / type = list(string)
(attribute
  (identifier) @_key (#eq? @_key "type")
  (expression) @type)
