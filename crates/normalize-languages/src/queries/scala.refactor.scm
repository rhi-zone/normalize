; Refactor node classification for Scala.

; Function definitions and their parameter list.
(function_definition (parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(call_expression (arguments) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). `val_definition` is immutable,
; `var_definition` mutable; a bare `assignment_expression` is reassignment.
(val_definition) @refactor.var_decl
(var_definition) @refactor.var_decl
(assignment_expression) @refactor.reassign

; Scope / block containers. `block` is a `{ ... }` expression body;
; `template_body` is a class/object/trait body; `compilation_unit` is the file.
(block) @refactor.scope @refactor.block
(template_body) @refactor.scope
(compilation_unit) @refactor.scope @refactor.block

; Statements. Scala models if/match/try/etc. as expressions; tag the
; control-flow forms so they are not treated as inline-able expressions.
(if_expression) @refactor.statement
(match_expression) @refactor.statement
(while_expression) @refactor.statement
(for_expression) @refactor.statement
(try_expression) @refactor.statement
