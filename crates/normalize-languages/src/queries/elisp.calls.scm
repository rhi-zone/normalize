; Emacs Lisp calls query
; @call — function application (list form where first element is the callee)
; @call.qualifier — not applicable (namespace is embedded in symbol name)
;
; In Emacs Lisp, function application is `(f args...)` — a list whose first
; element is the function name (a symbol). The tree-sitter grammar represents
; this as a `list` node with a leading `symbol` child.
;
; We use the `.` anchor to match only the first (leading) symbol child.

; Function call: (func args...)
(list
  .
  (symbol) @call)
