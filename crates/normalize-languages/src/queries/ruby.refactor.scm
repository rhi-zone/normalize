; Refactor node classification for Ruby.

; Method definitions and their parameter list. Methods defined without
; parentheses have no `method_parameters` node, so a bare `(method)` /
; `(singleton_method)` capture is included alongside the param-bearing form
; (duplicate function_def captures collapse into one node-ID set).
(method (method_parameters) @refactor.param_list) @refactor.function_def
(method) @refactor.function_def
(singleton_method (method_parameters) @refactor.param_list) @refactor.function_def
(singleton_method) @refactor.function_def

; Call expressions and their argument list.
(call (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). Ruby has no `let`/`const`; a bare
; assignment serves as both declaration and reassignment.
(assignment) @refactor.var_decl @refactor.reassign
(operator_assignment) @refactor.reassign

; Scope / block containers. `body_statement` is the method/class/begin body;
; its direct children are the statements. `do_block`/`block` are closure bodies.
(body_statement) @refactor.scope @refactor.block
(program) @refactor.scope @refactor.block
(do_block) @refactor.scope @refactor.block
(block) @refactor.scope @refactor.block

; Statements. Ruby has no `expression_statement` wrapper — bare expressions are
; statements directly — so only the control-flow forms are tagged (used to reject
; selecting a whole control-flow construct as an inline-able expression).
(if) @refactor.statement
(unless) @refactor.statement
(while) @refactor.statement
(until) @refactor.statement
(for) @refactor.statement
(case) @refactor.statement
(begin) @refactor.statement
(return) @refactor.statement
(break) @refactor.statement
(next) @refactor.statement
(redo) @refactor.statement
(yield) @refactor.statement
