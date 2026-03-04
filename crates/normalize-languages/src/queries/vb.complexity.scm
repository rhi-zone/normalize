; Complexity query for Visual Basic .NET
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(select_case_statement) @complexity
(while_statement) @complexity
(for_statement) @complexity
(for_each_statement) @complexity
(case_clause) @complexity

; Nesting nodes
(if_statement) @nesting
(select_case_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(for_each_statement) @nesting
(try_statement) @nesting
(method_declaration) @nesting
(property_declaration) @nesting
(class_block) @nesting
(module_block) @nesting
