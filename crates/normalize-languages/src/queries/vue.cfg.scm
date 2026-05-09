; Vue CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Vue grammar node types.
;
; Vue uses directive attributes (v-if, v-else-if, v-else, v-for) for
; template-level control flow. These are attributes on elements, not
; statement nodes. JavaScript logic inside <script> is handled by the JS grammar.
; Directive attributes are captured as branches/loops at the template level.

; ---------------------------------------------------------------------------
; v-if / v-else-if / v-else (branch via directives)
; ---------------------------------------------------------------------------

; v-if directive on an element
(element
  (start_tag
    (directive_attribute
      (directive_name) @_d
      (directive_argument)? @cfg.branch.condition
      (#match? @_d "^v-if$")))
) @cfg.branch

; v-for directive (loop over collection)
(element
  (start_tag
    (directive_attribute
      (directive_name) @_d
      (#match? @_d "^v-for$")))
) @cfg.loop
