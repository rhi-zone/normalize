; GraphQL calls query
; @call — field selections, directive names, fragment spreads
; @call.qualifier — object type context (parent field name) if applicable

; Field selection: user { name email } — field references are "calls" to schema fields
(field
  (name) @call)

; Directive application: @deprecated, @include(if: $var)
(directive
  (name) @call)

; Fragment spread: ...UserFragment — reference to a named fragment
(fragment_spread
  (fragment_name
    (name) @call))
