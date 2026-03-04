; R tags query
;
; R functions are assigned: name <- function(...) {...}
; In the tree-sitter grammar, assignments are binary_operator nodes.
; We match the left-assignment form with a function_definition RHS.

; name <- function(...)
(binary_operator
  lhs: (identifier) @name
  operator: "<-"
  rhs: (function_definition)) @definition.function

; name = function(...)
(binary_operator
  lhs: (identifier) @name
  operator: "="
  rhs: (function_definition)) @definition.function

; name <<- function(...)  (global assignment)
(binary_operator
  lhs: (identifier) @name
  operator: "<<-"
  rhs: (function_definition)) @definition.function
