; Prolog tags query
; Covers: predicate definitions (facts and rules) and directives
; Prolog programs consist of clause_term nodes (facts/rules) and directive_term nodes.
; Each clause is either a plain atom, a functional_notation (functor/arity),
; or an operator_notation with :- (head :- body rule).

; Simple fact: foo.
; The clause_term contains an atom directly.
(clause_term
  (atom) @name) @definition.function

; Compound fact or rule head: foo(X, Y). or foo(X) :- bar(X).
; The functional_notation has a function: (atom) field.
(clause_term
  (functional_notation
    function: (atom) @name)) @definition.function

; Rule with :- operator: head :- body.
; The operator_notation's first child (before :-) is the head.
; Head can be atom or functional_notation.
(clause_term
  (operator_notation
    (atom) @name)) @definition.function

(clause_term
  (operator_notation
    (functional_notation
      function: (atom) @name))) @definition.function

; Directives: :- module(foo, [...]).
; directive_term contains a directive_head node or functional_notation.
(directive_term
  (functional_notation
    function: (atom) @name)) @definition.module
