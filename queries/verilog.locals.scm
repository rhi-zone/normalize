; Verilog locals.scm
; Modules, tasks, and functions create scopes. Port declarations, wire/reg
; declarations, and task/function parameters use simple_identifier nodes.

; Scopes
; ------

[
  (module_declaration)
  (task_body_declaration)
  (function_body_declaration)
] @local.scope

; Definitions
; -----------

; Module name
(module_header
  (simple_identifier) @local.definition)

; Module ports (input/output/inout declarations)
(port_identifier
  (simple_identifier) @local.definition)

; Wire/net declarations
(net_decl_assignment
  (simple_identifier) @local.definition)

; Reg/logic/variable declarations
(variable_decl_assignment
  (simple_identifier) @local.definition)

; Task name
(task_identifier
  (simple_identifier) @local.definition)

; Task/function parameters
(tf_port_item1
  (port_identifier
    (simple_identifier) @local.definition))

; Block item declarations (local variables in tasks/functions)
(block_item_declaration
  (data_declaration
    (list_of_variable_decl_assignments
      (variable_decl_assignment
        (simple_identifier) @local.definition))))

; References
; ----------

(simple_identifier) @local.reference
