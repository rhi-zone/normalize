; Common Lisp tags query
;
; In the CL grammar, some definition forms have dedicated grammar nodes (defun,
; defgeneric) while others use plain list_lit with a sym_lit keyword.

; (defun name ...) and (defgeneric name ...) — both use the `defun` grammar node
(defun
  (defun_header
    function_name: (sym_lit) @name)) @definition.function

; (defmethod name ...) — uses list_lit with sym_lit keyword
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defmethod")
  .
  (sym_lit) @name) @definition.method

; (defmacro name ...) — uses list_lit with sym_lit keyword
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defmacro")
  .
  (sym_lit) @name) @definition.macro

; (defclass name ...)
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defclass")
  .
  (_) @name) @definition.class

; (defstruct name ...)
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defstruct")
  .
  (sym_lit) @name) @definition.class

; (defpackage name ...)
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defpackage")
  .
  (_) @name) @definition.module

; (deftype name ...)
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "deftype")
  .
  (sym_lit) @name) @definition.type

; (defconstant name ...)
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defconstant")
  .
  (sym_lit) @name) @definition.constant

; (defparameter name ...)
(list_lit
  .
  (sym_lit) @_kw (#eq? @_kw "defparameter")
  .
  (sym_lit) @name) @definition.constant
