; Emacs Lisp locals.scm
; defun/defmacro use dedicated function_definition/macro_definition nodes.
; let/let*/lambda/defvar/defconst/setq are special_form nodes; the keyword
; is an unnamed child whose node-kind equals the keyword text.

; Scopes
; ------

(function_definition) @local.scope
(macro_definition) @local.scope

; let/let* special forms
(special_form
  ["let" "let*"]
  (list)) @local.scope

; lambda special form
(special_form "lambda" (list)) @local.scope

; Definitions
; -----------

; defun name and parameters
(function_definition
  name: (symbol) @local.definition)

(function_definition
  parameters: (list (symbol) @local.definition))

; defmacro name and parameters
(macro_definition
  name: (symbol) @local.definition)

(macro_definition
  parameters: (list (symbol) @local.definition))

; let/let* binding pairs: (let ((x val) ...) body) — first sym of each pair
(special_form
  ["let" "let*"]
  (list (list . (symbol) @local.definition)))

; lambda parameters: (lambda (x y) body)
(special_form "lambda"
  . _ .
  (list (symbol) @local.definition))

; defvar/defconst: first named child after keyword is the variable name
(special_form "defvar"
  . (symbol) @local.definition)

(special_form "defconst"
  . (symbol) @local.definition)

; condition-case error variable: (condition-case err body handler)
(special_form "condition-case"
  . (symbol) @local.definition)

; References
; ----------

(symbol) @local.reference
