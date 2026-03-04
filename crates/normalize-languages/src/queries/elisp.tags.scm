; Emacs Lisp tags query
;
; Emacs Lisp uses list nodes for all forms.
; The first child symbol names the form.

; (defun name ...)
(list
  (symbol) @_kw (#eq? @_kw "defun")
  .
  (symbol) @name) @definition.function

; (defsubst name ...)
(list
  (symbol) @_kw (#eq? @_kw "defsubst")
  .
  (symbol) @name) @definition.function

; (cl-defun name ...)
(list
  (symbol) @_kw (#eq? @_kw "cl-defun")
  .
  (symbol) @name) @definition.function

; (defmacro name ...)
(list
  (symbol) @_kw (#eq? @_kw "defmacro")
  .
  (symbol) @name) @definition.macro

; (cl-defmacro name ...)
(list
  (symbol) @_kw (#eq? @_kw "cl-defmacro")
  .
  (symbol) @name) @definition.macro

; (defvar name ...)
(list
  (symbol) @_kw (#eq? @_kw "defvar")
  .
  (symbol) @name) @definition.constant

; (defconst name ...)
(list
  (symbol) @_kw (#eq? @_kw "defconst")
  .
  (symbol) @name) @definition.constant

; (defcustom name ...)
(list
  (symbol) @_kw (#eq? @_kw "defcustom")
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
