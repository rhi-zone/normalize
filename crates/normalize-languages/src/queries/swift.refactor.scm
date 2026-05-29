; Refactor node classification for Swift.

; Function definitions. NOTE: the Swift tree-sitter grammar does not model a
; single parameter-list container node — parameters are loose `(parameter)`
; children between anonymous `(` / `)` tokens — so there is no
; `@refactor.param_list` capture. extract-function / inline-variable /
; introduce-variable work; add-parameter (which needs a single param-list span)
; is not supported for Swift definitions until the grammar exposes one.
(function_declaration) @refactor.function_def

; Call expressions and their argument list. Swift nests `value_arguments`
; inside a `call_suffix`.
(call_expression (call_suffix (value_arguments) @refactor.arg_list)) @refactor.call

; Variable declarations (inline-variable). `property_declaration` covers both
; `let` and `var`; a bare `assignment` is reassignment.
(property_declaration) @refactor.var_decl
(assignment) @refactor.reassign

; Scope / block containers. `statements` is the statement list whose direct
; children are the statements; the body nodes are scopes.
(statements) @refactor.scope @refactor.block
(function_body) @refactor.scope
(class_body) @refactor.scope
(source_file) @refactor.scope

; Statements.
(if_statement) @refactor.statement
(guard_statement) @refactor.statement
(while_statement) @refactor.statement
(repeat_while_statement) @refactor.statement
(for_statement) @refactor.statement
(switch_statement) @refactor.statement
(do_statement) @refactor.statement
(control_transfer_statement) @refactor.statement
