; D calls query
; @call — function/method call identifiers
;
; In D, function calls appear as postfix_expression where the function name
; is represented as a qualified_identifier child.

; Function and method calls: func(args), obj.method(args)
(postfix_expression
  (qualified_identifier) @call)
