; Refactor node classification for PHP.

; Function/method definitions and their parameter list.
(function_definition (formal_parameters) @refactor.param_list) @refactor.function_def
(method_declaration (formal_parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(function_call_expression (arguments) @refactor.arg_list) @refactor.call
(member_call_expression (arguments) @refactor.arg_list) @refactor.call
(scoped_call_expression (arguments) @refactor.arg_list) @refactor.call
(object_creation_expression (arguments) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). PHP has no `let`/`const` for locals;
; a bare `$x = ...` assignment serves as both declaration and reassignment.
(assignment_expression) @refactor.var_decl @refactor.reassign
(augmented_assignment_expression) @refactor.reassign

; Scope / block containers.
(compound_statement) @refactor.scope @refactor.block
(declaration_list) @refactor.scope
(program) @refactor.scope @refactor.block

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(if_statement) @refactor.statement
(for_statement) @refactor.statement
(foreach_statement) @refactor.statement
(while_statement) @refactor.statement
(do_statement) @refactor.statement
(switch_statement) @refactor.statement
(try_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
(echo_statement) @refactor.statement
