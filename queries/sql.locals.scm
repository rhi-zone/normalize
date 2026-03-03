; SQL locals.scm
; SQL does not have traditional block scoping. Statement-level constructs
; that introduce names: CTEs (WITH x AS ...) and table/subquery aliases
; (FROM tbl AS t). Column aliases (SELECT 1 AS col) are also captured.

; Scopes
; ------

; Each statement is its own scope
(statement) @local.scope

; Definitions
; -----------

; CTE name: first identifier child of cte node (no named field in grammar)
(cte . (identifier) @local.definition)

; Table/subquery alias in FROM clause
(relation
  alias: (identifier) @local.definition)

; Column alias: identifier after AS in a select term
; Note: term has no named fields; the alias is always the last identifier.
; Capture all identifiers in terms as definitions (over-captures literals).
(select_expression
  (term
    (keyword_as)
    (identifier) @local.definition))

; References
; ----------

(identifier) @local.reference
