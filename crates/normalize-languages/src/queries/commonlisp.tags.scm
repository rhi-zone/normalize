; Common Lisp tags query
;
; Common Lisp forms are list_lit nodes with a leading sym_lit naming the form.

; (defun name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defun")
  .
  (sym_lit) @name) @definition.function

; (defgeneric name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defgeneric")
  .
  (sym_lit) @name) @definition.function

; (defmethod name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defmethod")
  .
  (sym_lit) @name) @definition.method

; (defmacro name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defmacro")
  .
  (sym_lit) @name) @definition.macro

; (defclass name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defclass")
  .
  (sym_lit) @name) @definition.class

; (defstruct name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defstruct")
  .
  (sym_lit) @name) @definition.class

; (defpackage name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defpackage")
  .
  (_) @name) @definition.module

; (deftype name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "deftype")
  .
  (sym_lit) @name) @definition.type

; (defconstant name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defconstant")
  .
  (sym_lit) @name) @definition.constant

; (defparameter name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defparameter")
  .
  (sym_lit) @name) @definition.constant
