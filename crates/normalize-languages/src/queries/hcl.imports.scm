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
        (quoted_template) @import.path)))) @import
