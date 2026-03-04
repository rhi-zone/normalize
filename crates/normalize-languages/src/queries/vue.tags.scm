; Vue tags — top-level script, template, and style blocks
; Vue SFCs embed JS in <script>, HTML in <template>, and CSS in <style>.
; The blocks themselves are captured as containers; symbol extraction
; happens via language injection in the embedded content.

(script_element
  (start_tag
    (tag_name) @name)) @definition.module

(style_element
  (start_tag
    (tag_name) @name)) @definition.module

(template_element
  (start_tag
    (tag_name) @name)) @definition.module
