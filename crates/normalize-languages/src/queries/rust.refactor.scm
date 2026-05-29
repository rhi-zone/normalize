; Refactor node classification for Rust.
; Captures the structural node kinds the refactoring recipes need, replacing
; hardcoded `match grammar` / kind-union dispatch in normalize-refactor.

; Function/method definitions and their parameter list.
(function_item (parameters) @refactor.param_list) @refactor.function_def
(function_signature_item (parameters) @refactor.param_list) @refactor.function_def

; Call expressions and their argument list.
(call_expression (arguments) @refactor.arg_list) @refactor.call

; Variable declarations (inline-variable).
(let_declaration) @refactor.var_decl

; Reassignment targets (inline-variable reassignment check).
(assignment_expression) @refactor.reassign
(compound_assignment_expr) @refactor.reassign

; Scope / block containers.
(block) @refactor.scope @refactor.block
(source_file) @refactor.scope
