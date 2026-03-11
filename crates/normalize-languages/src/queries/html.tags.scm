; HTML elements as symbols.
; Elements with children become Modules via refine_kind; leaf elements stay as Variables.

(element
  (start_tag
    (tag_name) @name)) @definition.var

(element
  (self_closing_tag
    (tag_name) @name)) @definition.var

(script_element
  (start_tag
    (tag_name) @name)) @definition.var

(style_element
  (start_tag
    (tag_name) @name)) @definition.var
