; XML elements as symbols.
; Elements with children become Modules via refine_kind; leaf elements stay as Variables.

(element
  (STag
    (Name) @name)) @definition.var

(element
  (EmptyElemTag
    (Name) @name)) @definition.var
