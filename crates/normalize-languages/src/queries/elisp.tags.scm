; Emacs Lisp tags query
;
; In the elisp grammar, (defun name ...) parses as a top-level function_definition
; node with a name: symbol field. Similarly (defmacro ...) parses as macro_definition.
; Other forms remain as list nodes with leading symbol keywords.

; (defun name ...)
; (defsubst name ...)
; (cl-defun name ...)
(function_definition
  name: (symbol) @name) @definition.function

; (defmacro name ...)
; (cl-defmacro name ...)
(macro_definition
  name: (symbol) @name) @definition.macro

; (defvar name ...) — parses as special_form with symbol as first named child
(special_form
  .
  (symbol) @name) @definition.constant

; (cl-defstruct name ...)
(list
  (symbol) @_kw (#eq? @_kw "cl-defstruct")
  .
  (symbol) @name) @definition.class

; (defclass name ...)  — EIEIO
(list
  (symbol) @_kw (#eq? @_kw "defclass")
  .
  (symbol) @name) @definition.class
