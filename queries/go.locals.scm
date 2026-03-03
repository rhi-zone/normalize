; Scopes
; ------

[
  (function_declaration)
  (method_declaration)
  (func_literal)
  (block)
  (if_statement)
  (for_statement)
  (select_statement)
  (expression_case)
  (type_case)
] @local.scope

; Definitions
; -----------

; Function names
(function_declaration
  name: (identifier) @local.definition)

; Method names
(method_declaration
  name: (field_identifier) @local.definition)

; Function and method parameters
(parameter_declaration
  name: (identifier) @local.definition)

; Variadic parameter
(variadic_parameter_declaration
  name: (identifier) @local.definition)

; Short variable declarations (:=)
(short_var_declaration
  left: (expression_list
    (identifier) @local.definition))

; Range clause variables
(range_clause
  left: (expression_list
    (identifier) @local.definition))

; Const/var declarations
(var_spec
  name: (identifier) @local.definition)

(const_spec
  name: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
