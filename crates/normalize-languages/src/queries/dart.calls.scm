; Dart calls query
; @call — call expression identifiers
;
; In Dart, function calls appear as: identifier followed by selector(argument_part)
; A selector containing argument_part represents the call.
; The identifier precedes the selector as a sibling in the parent node.

; Simple function call: func()
((identifier) @call
 .
 (selector
   (argument_part)))
