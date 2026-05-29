; Refactor node classification for Lua.

; Function definitions and their parameter list. `function_declaration` covers
; both `function name() ... end` and `local function name() ... end`;
; `function_definition` is the anonymous `function() ... end` expression.
(function_declaration (parameters) @refactor.param_list) @refactor.function_def
(function_definition (parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(function_call (arguments) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). `local x = ...` is the declaration;
; a bare `x = ...` assignment is reassignment.
(variable_declaration) @refactor.var_decl
(assignment_statement) @refactor.reassign

; Scope / block containers. `block` is any do/if/while/for/function body; `chunk`
; is the top-level file body.
(block) @refactor.scope @refactor.block
(chunk) @refactor.scope @refactor.block

; Statements. Lua has no `expression_statement` wrapper — a bare call is a
; statement directly — so only the control-flow forms are tagged.
(if_statement) @refactor.statement
(while_statement) @refactor.statement
(for_statement) @refactor.statement
(repeat_statement) @refactor.statement
(do_statement) @refactor.statement
(return_statement) @refactor.statement
(break_statement) @refactor.statement
(goto_statement) @refactor.statement
