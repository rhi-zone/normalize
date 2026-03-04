; Complexity query for Scheme
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Scheme is a Lisp; all forms are list nodes. Complexity comes from
; if/when/unless/cond/case/do/let-loop forms. All are represented as
; list with a leading symbol naming the form.

; Complexity nodes — any list form
(list) @complexity

; Nesting nodes
(list) @nesting
