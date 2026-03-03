; Prolog locals.scm
; Prolog variables are unification variables scoped to their clause.
; Each clause_term is a scope; all variable_term nodes are definitions
; and references within that clause. The anonymous variable _ is excluded.

; Scopes
; ------

(clause_term) @local.scope

; Definitions and References
; --------------------------

; Every named variable is both a definition site (first occurrence)
; and a reference site (later occurrences) within the clause scope.
; The scope engine's resolve step handles deduplication.
((variable_term) @local.definition
 (#not-eq? @local.definition "_"))

((variable_term) @local.reference
 (#not-eq? @local.reference "_"))
