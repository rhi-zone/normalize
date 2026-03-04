; Zig complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Zig's tree-sitter grammar uses PascalCase node names (e.g. IfStatement,
; ForStatement) inherited from its grammar source.

; Complexity nodes
(IfStatement) @complexity
(ForStatement) @complexity
(WhileStatement) @complexity
(SwitchExpr) @complexity
(ErrorUnionExpr) @complexity
(BinaryExpr) @complexity

; Nesting nodes
(IfStatement) @nesting
(ForStatement) @nesting
(WhileStatement) @nesting
(SwitchExpr) @nesting
(FnProto) @nesting
(ContainerDecl) @nesting
