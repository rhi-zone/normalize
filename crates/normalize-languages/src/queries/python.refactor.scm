; Refactor node classification for Python.

; Function/method definitions and their parameter list.
(function_definition (parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(call (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). Python has no dedicated `let`/`const`;
; a bare assignment serves as both declaration and reassignment — the recipe
; disambiguates by binding position at runtime.
(assignment) @refactor.var_decl @refactor.reassign
(augmented_assignment) @refactor.reassign

; Scope / block containers.
(block) @refactor.scope @refactor.block
(module) @refactor.scope @refactor.block

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(assert_statement) @refactor.statement
(pass_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
(delete_statement) @refactor.statement
(import_statement) @refactor.statement
(import_from_statement) @refactor.statement
(raise_statement) @refactor.statement
(global_statement) @refactor.statement
(nonlocal_statement) @refactor.statement
