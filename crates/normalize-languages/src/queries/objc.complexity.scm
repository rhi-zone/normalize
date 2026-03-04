; Complexity query for Objective-C
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth

; Complexity nodes
(if_statement) @complexity
(switch_statement) @complexity
(while_statement) @complexity
(for_statement) @complexity

; Nesting nodes
(if_statement) @nesting
(switch_statement) @nesting
(while_statement) @nesting
(for_statement) @nesting
(try_statement) @nesting
(method_declaration) @nesting
(function_definition) @nesting
(class_interface) @nesting
(class_implementation) @nesting
