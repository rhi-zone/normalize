; Typst type references
; Typst uses tagged parameters for type annotations in function definitions:
;   #let foo(x: int, y: str) = ...
; The grammar models this as (let (call (group (tagged (ident) (ident)))))
; where the second ident in tagged is the type. We anchor to `let` to avoid
; matching named arguments in function calls like #foo(style: bold).
; No return type annotations exist in the grammar (-> causes ERROR).

; Parameter type annotations in let-bound function definitions
(let
  (call
    (group
      (tagged
        (ident)
        (ident) @type.reference))))
