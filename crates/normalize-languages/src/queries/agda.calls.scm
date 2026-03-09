; Agda calls query
; @call — call/application expression nodes
; @call.qualifier — module qualifier for qualified calls
;
; Agda uses juxtaposition for function application (like Haskell).
; `f x y` is represented as an `expr` with multiple children where the
; first is the function. Module application uses `module_application`.

; Module application: module M = SomeModule arg
(module_application
  (module_name) @call)

; Function application in expressions: the `expr` node's first child is
; typically the applied function (a qid).
; Use the expr node to capture application — first atom's qid is the callee.
(expr
  .
  (atom
    .
    (qid) @call))
