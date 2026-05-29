; Refactor node classification for Java.

; Method/constructor definitions and their parameter list.
(method_declaration (formal_parameters) @refactor.param_list) @refactor.function_def
(constructor_declaration (formal_parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(method_invocation (argument_list) @refactor.arg_list) @refactor.call
(object_creation_expression (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). Java has no `let`/`const`; a local
; variable declaration introduces the binding.
(local_variable_declaration) @refactor.var_decl

; Reassignment targets (inline-variable reassignment check). `=` and compound
; assignment both surface as `assignment_expression`.
(assignment_expression) @refactor.reassign
(update_expression) @refactor.reassign

; Scope / block containers.
(block) @refactor.scope @refactor.block
(program) @refactor.scope @refactor.block
(class_body) @refactor.scope

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(if_statement) @refactor.statement
(for_statement) @refactor.statement
(enhanced_for_statement) @refactor.statement
(while_statement) @refactor.statement
(do_statement) @refactor.statement
(switch_expression) @refactor.statement
(try_statement) @refactor.statement
(throw_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
(yield_statement) @refactor.statement
