; Refactor node classification for Groovy.

; Function definitions and their parameter list.
(function_definition (parameter_list) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(function_call (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). `declaration` is `def x = ...` /
; `Type x = ...`; a bare `assignment` is reassignment.
(declaration) @refactor.var_decl
(assignment) @refactor.reassign

; Scope / block containers. Groovy uses `closure` (`{ ... }`) for function and
; control-flow bodies; its direct children are the statements.
(closure) @refactor.scope @refactor.block
(source_file) @refactor.scope @refactor.block

; Statements. Groovy's grammar does not model return/break/continue as distinct
; statement nodes (they surface as keyword tokens), so only the compound
; control-flow forms are tagged.
(if_statement) @refactor.statement
(while_loop) @refactor.statement
(do_while_loop) @refactor.statement
(for_in_loop) @refactor.statement
(switch_statement) @refactor.statement
(try_statement) @refactor.statement
