; Scheme locals.scm
; Scheme is homoiconic: all special forms are `list` nodes distinguished
; by their leading `symbol`. Text predicates (#eq?, #any-of?) on named
; `symbol` captures are evaluated by QueryCursor::matches.

; Scopes
; ------

; (define (f ...) body) creates a function scope
((list . (symbol) @_kw . (list)) @local.scope
 (#eq? @_kw "define"))

; lambda/let/let*/letrec/letrec*/do create scopes
((list . (symbol) @_kw) @local.scope
 (#any-of? @_kw "lambda" "let" "let*" "letrec" "letrec*" "do"))

; Definitions
; -----------

; Variable define: (define x val) — adjacent symbol after keyword
((list . (symbol) @_kw . (symbol) @local.definition)
 (#eq? @_kw "define"))

; Function define: (define (f x y) body) — all symbols in formals list
; (dot after @_kw restricts to the second named child = formals list)
((list . (symbol) @_kw . (list (symbol) @local.definition))
 (#eq? @_kw "define"))

; Lambda params: (lambda (x y) body)
((list . (symbol) @_kw . (list (symbol) @local.definition))
 (#eq? @_kw "lambda"))

; Let/letrec bindings: (let ((x val) ...) body) — first sym of each pair
((list . (symbol) @_kw . (list (list . (symbol) @local.definition)))
 (#any-of? @_kw "let" "let*" "letrec" "letrec*"))

; Do loop vars: (do ((i 0 step) ...) test body) — first sym of each var spec
((list . (symbol) @_kw . (list (list . (symbol) @local.definition)))
 (#eq? @_kw "do"))

; References
; ----------

(symbol) @local.reference
