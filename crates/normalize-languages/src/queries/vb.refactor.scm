; Refactor node classification for Visual Basic.

; Method (Sub/Function) definitions and their parameter list.
(method_declaration (parameter_list) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(invocation (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). `Dim x As T = ...` is the
; declaration. NOTE: VB has no distinct reassignment node — a bare `x = expr`
; reassignment is parsed as a `call_statement` (the grammar cannot
; disambiguate assignment from an equality-valued call), so there is no
; `@refactor.reassign` capture.
(dim_statement) @refactor.var_decl

; Scope / block containers. A `method_declaration` directly contains its
; `(statement ...)` children (there is no inner block node), so it is both the
; scope and the statement-list block.
(method_declaration) @refactor.scope @refactor.block
(module_block) @refactor.scope @refactor.block
(class_block) @refactor.scope
(source_file) @refactor.scope @refactor.block

; Statements. VB wraps each statement in a `(statement ...)` node.
(statement) @refactor.statement
