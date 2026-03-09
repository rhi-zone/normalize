; HCL imports query
; @import       — the entire module block (for line number)
; @import.path  — the source attribute value

; module "vpc" {
;   source = "./modules/vpc"
; }
(block
  (identifier) @_type
  (#eq? @_type "module")
  (body
    (attribute
      (identifier) @_attr
      (#eq? @_attr "source")
      (expression
        (literal_value
          (string_lit) @import.path))))) @import

; Also match template expressions (interpolated strings)
(block
  (identifier) @_type
  (#eq? @_type "module")
  (body
    (attribute
      (identifier) @_attr
      (#eq? @_attr "source")
      (expression
        (template_expr
          (quoted_template) @import.path))))) @import
