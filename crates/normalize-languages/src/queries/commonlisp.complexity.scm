; Complexity query for Common Lisp
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Common Lisp is a Lisp; all forms are list_lit nodes. Complexity comes from
; if/when/unless/cond/case/ecase/do/dolist/dotimes/loop forms. All are
; represented as list_lit with a leading symbol naming the form.

; Complexity nodes — any list form
(list_lit) @complexity

; Nesting nodes
(list_lit) @nesting
