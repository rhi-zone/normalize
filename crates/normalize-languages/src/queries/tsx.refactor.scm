; Refactor node classification for TSX (shares the TypeScript grammar).

; Function/method definitions and their parameter list.
(function_declaration (formal_parameters) @refactor.param_list) @refactor.function_def
(function_expression (formal_parameters) @refactor.param_list) @refactor.function_def
(method_definition (formal_parameters) @refactor.param_list) @refactor.function_def
(arrow_function (formal_parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(call_expression (arguments) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable): const / let / var.
(lexical_declaration) @refactor.var_decl
(variable_declaration) @refactor.var_decl

; Reassignment targets (inline-variable reassignment check).
(assignment_expression) @refactor.reassign
(augmented_assignment_expression) @refactor.reassign

; Scope / block containers.
(statement_block) @refactor.scope @refactor.block
(program) @refactor.scope @refactor.block
(class_body) @refactor.scope
(enum_body) @refactor.scope

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(throw_statement) @refactor.statement
(if_statement) @refactor.statement
(while_statement) @refactor.statement
(for_statement) @refactor.statement
(for_in_statement) @refactor.statement
(switch_statement) @refactor.statement
(try_statement) @refactor.statement
