; Complexity query for HCL (Terraform/HashiCorp Configuration Language)
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; HCL complexity comes from conditional expressions, for expressions,
; and dynamic blocks.

; Complexity nodes
(conditional) @complexity
(for_tuple_expr) @complexity
(for_object_expr) @complexity
(block) @complexity

; Nesting nodes
(block) @nesting
(conditional) @nesting
(for_tuple_expr) @nesting
(for_object_expr) @nesting
