; Scopes
; ------

; let...in expression scopes the binding for its body
[
  (let_expression)
  (fun_expression)
  (match_case)
] @local.scope

; let_binding itself is a scope: covers function body access to params
(let_binding) @local.scope

; Definitions
; -----------

; Simple let binding: let x = ... (value_name in pattern position)
(let_binding
  pattern: (value_name) @local.definition)

; Curried function parameters: let f x y = ... (value_pattern children)
(let_binding
  (value_pattern) @local.definition)

; fun expression parameters: fun x -> ...
(parameter
  pattern: (value_pattern) @local.definition)

; References
; ----------

(value_path . (value_name) @local.reference)
