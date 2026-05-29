; Refactor node classification for Dart.

; Function/method definitions and their parameter list. A method wraps a
; `function_signature`; both expose `formal_parameter_list`.
(function_signature (formal_parameter_list) @refactor.param_list) @refactor.function_def

; NOTE: the Dart tree-sitter grammar has no dedicated call-expression node — a
; call is an `identifier` followed by a sibling `selector (argument_part
; (arguments))` inside an `expression_statement`, with no node grouping the
; callee name with its argument list. There is therefore no sound
; `@refactor.call` / `@refactor.arg_list` capture (the name and args are
; siblings, not parent/child). extract-function / inline-variable /
; introduce-variable work; add-parameter updates the definition signature but
; cannot rewrite call sites until the grammar models a call node.

; Variable declarations (inline-variable). `local_variable_declaration` is
; `var x = ...` / `int x = ...`; `assignment_expression` is reassignment.
(local_variable_declaration) @refactor.var_decl
(assignment_expression) @refactor.reassign

; Scope / block containers.
(block) @refactor.scope @refactor.block
(function_body) @refactor.scope
(class_body) @refactor.scope
(program) @refactor.scope @refactor.block

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(if_statement) @refactor.statement
(for_statement) @refactor.statement
(while_statement) @refactor.statement
(do_statement) @refactor.statement
(switch_statement) @refactor.statement
(try_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
