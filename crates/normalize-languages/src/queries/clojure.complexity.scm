; Complexity query for Clojure
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Clojure is a Lisp; all forms are list_lit nodes. Complexity comes from
; if/when/cond/case/condp/loop/doseq/for/dotimes forms as well as boolean
; short-circuit operators (and/or). All are represented as list_lit with
; a leading symbol node naming the form.

; Complexity nodes — any list form (if, cond, case, when, for, etc.)
(list_lit) @complexity

; Nesting nodes — same forms introduce new logical scopes
(list_lit) @nesting
