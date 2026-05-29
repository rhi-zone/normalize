; Refactor node classification for C#.

; Method / constructor / local-function definitions and their parameter list.
(method_declaration (parameter_list) @refactor.param_list) @refactor.function_def
(constructor_declaration (parameter_list) @refactor.param_list) @refactor.function_def
(local_function_statement (parameter_list) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(invocation_expression (argument_list) @refactor.arg_list) @refactor.call
(object_creation_expression (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). C# has no `let`; a local declaration
; statement introduces the binding; assignment is reassignment.
(local_declaration_statement) @refactor.var_decl
(assignment_expression) @refactor.reassign

; Scope / block containers.
(block) @refactor.scope @refactor.block
(declaration_list) @refactor.scope
(compilation_unit) @refactor.scope @refactor.block

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
(throw_statement) @refactor.statement
(using_statement) @refactor.statement
(lock_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
(yield_statement) @refactor.statement
