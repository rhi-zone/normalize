; Complexity query for Emacs Lisp
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Emacs Lisp is a Lisp; all forms are list nodes. Complexity comes from
; if/when/unless/cond/case/cl-case/while/dolist/dotimes/cl-loop forms and
; boolean short-circuit operators (and/or). All are represented as list nodes.

; Complexity nodes — any list form (if, cond, case, when, unless, etc.)
(list) @complexity

; Nesting nodes
(list) @nesting
