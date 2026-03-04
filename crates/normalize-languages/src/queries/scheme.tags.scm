; Scheme tags query
;
; Scheme uses list nodes for all forms.
; The first child symbol names the form.

; (define (name args) body)
(list
  (symbol) @_kw (#eq? @_kw "define")
  .
  (list
    .
    (symbol) @name)) @definition.function

; (define name (lambda ...))
(list
  (symbol) @_kw (#eq? @_kw "define")
  .
  (symbol) @name
  .
  (list
    (symbol) @_lambda (#eq? @_lambda "lambda"))) @definition.function

; (define name value)  — constant/variable
(list
  (symbol) @_kw (#eq? @_kw "define")
  .
  (symbol) @name) @definition.constant

; (define-record-type name ...)
(list
  (symbol) @_kw (#eq? @_kw "define-record-type")
  .
  (symbol) @name) @definition.class

; (define-syntax name ...)
(list
  (symbol) @_kw (#eq? @_kw "define-syntax")
  .
  (symbol) @name) @definition.macro

; (define-library name ...)
(list
  (symbol) @_kw (#eq? @_kw "define-library")
  .
  (_) @name) @definition.module
