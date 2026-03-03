; Common Lisp locals.scm
; defun/defmacro/lambda forms use the dedicated `defun` AST node.
; let/let*/flet/labels/defvar/defparameter are plain list_lit nodes
; distinguished by text predicate on the leading sym_lit keyword.

; Scopes
; ------

; defun/defmacro/lambda body is a scope
(defun) @local.scope

; let/let*/flet/labels create scopes
((list_lit . (sym_lit) @_kw) @local.scope
 (#any-of? @_kw "let" "let*" "flet" "labels" "do" "dotimes" "dolist"))

; Definitions
; -----------

; Function/macro name: sym_lit adjacent to defun_keyword in header
; (not present for lambda forms which have no name)
(defun_header (defun_keyword) . (sym_lit) @local.definition)

; Parameters: all sym_lits in the lambda_list (the params list_lit)
(defun_header (list_lit (sym_lit) @local.definition))

; let/let* bindings: (let ((x val) ...) body) — first sym_lit of each pair
((list_lit . (sym_lit) @_kw . (list_lit (list_lit . (sym_lit) @local.definition)))
 (#any-of? @_kw "let" "let*"))

; flet/labels function bindings: (flet ((f (x) body)) body)
((list_lit . (sym_lit) @_kw . (list_lit (list_lit . (sym_lit) @local.definition)))
 (#any-of? @_kw "flet" "labels"))

; defvar/defparameter/defconstant: second sym_lit is the variable name
((list_lit . (sym_lit) @_kw . (sym_lit) @local.definition)
 (#any-of? @_kw "defvar" "defparameter" "defconstant"))

; loop for: variable field of for_clause
(for_clause variable: (sym_lit) @local.definition)

; dolist/dotimes: first sym_lit of the binding list
((list_lit . (sym_lit) @_kw . (list_lit . (sym_lit) @local.definition))
 (#any-of? @_kw "dolist" "dotimes"))

; References
; ----------

(sym_lit) @local.reference
