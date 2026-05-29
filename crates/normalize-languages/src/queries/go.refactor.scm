; Refactor node classification for Go.

; Function/method definitions and their parameter list. The `parameters:` field
; name is required for methods: a bare `(parameter_list)` would also match the
; receiver list and the multi-return result list.
(function_declaration parameters: (parameter_list) @refactor.param_list) @refactor.function_def
(method_declaration parameters: (parameter_list) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(call_expression (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). Go has both `var x = ...` and the
; short `x := ...` form; both introduce a binding.
(var_declaration) @refactor.var_decl
(short_var_declaration) @refactor.var_decl

; Reassignment targets (inline-variable reassignment check).
(assignment_statement) @refactor.reassign

; Scope / block containers.
(block) @refactor.scope @refactor.block
(source_file) @refactor.scope

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(if_statement) @refactor.statement
(for_statement) @refactor.statement
(go_statement) @refactor.statement
(defer_statement) @refactor.statement
(send_statement) @refactor.statement
(labeled_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
(expression_switch_statement) @refactor.statement
(type_switch_statement) @refactor.statement
(select_statement) @refactor.statement
