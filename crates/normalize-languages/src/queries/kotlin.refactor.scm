; Refactor node classification for Kotlin.

; Function definitions and their parameter list.
(function_declaration (function_value_parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list. Kotlin nests `value_arguments`
; inside a `call_suffix`; the arg-list capture targets the inner node.
(call_expression (call_suffix (value_arguments) @refactor.arg_list)) @refactor.call

; Variable declarations (inline-variable). `property_declaration` covers both
; `val` and `var`; a bare `assignment` is reassignment.
(property_declaration) @refactor.var_decl
(assignment) @refactor.reassign

; Scope / block containers. `statements` is the statement list whose direct
; children are the statements (used to locate statement boundaries); the body
; nodes are scopes.
(statements) @refactor.scope @refactor.block
(function_body) @refactor.scope
(class_body) @refactor.scope
(control_structure_body) @refactor.scope
(source_file) @refactor.scope

; Statements. Kotlin models if/when/try/etc. as expressions; tag the
; control-flow forms so they are not treated as inline-able expressions.
(if_expression) @refactor.statement
(when_expression) @refactor.statement
(while_statement) @refactor.statement
(do_while_statement) @refactor.statement
(for_statement) @refactor.statement
(try_expression) @refactor.statement
(jump_expression) @refactor.statement
