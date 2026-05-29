; Refactor node classification for D.

; Function definitions and their parameter list. The parameter list is nested
; under `func_declarator > func_declarator_suffix > parameters`.
(func_declaration (func_declarator (func_declarator_suffix (parameters) @refactor.param_list))) @refactor.function_def

; Call expressions and their argument list. A call is a `postfix_expression`
; with an `argument_list`.
(postfix_expression (argument_list) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable). `declaration_statement` is the
; statement-level declaration (`int x = ...`).
(declaration_statement) @refactor.var_decl

; Reassignment. NOTE: the D tree-sitter grammar struggles with bare assignment
; statements (it sometimes splits `x = y + 1;` into an `alias_assign` plus a
; partial `expression_statement`), so reassignment detection is best-effort via
; the `assign_expression` node where the grammar produces it.
(assign_expression) @refactor.reassign

; Scope / block containers.
(block_statement) @refactor.scope @refactor.block
(statement_list) @refactor.scope @refactor.block
(module) @refactor.scope @refactor.block

; Statements.
(expression_statement) @refactor.statement
(return_statement) @refactor.statement
(if_statement) @refactor.statement
(while_statement) @refactor.statement
(for_statement) @refactor.statement
(foreach_statement) @refactor.statement
(switch_statement) @refactor.statement
(do_statement) @refactor.statement
(try_statement) @refactor.statement
(break_statement) @refactor.statement
(continue_statement) @refactor.statement
