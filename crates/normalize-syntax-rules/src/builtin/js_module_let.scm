# ---
# id = "js/module-let"
# severity = "info"
# tags = ["architecture", "style"]
# message = "Module-level let - consider const, or encapsulate mutable state in a class or function"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# `let` at the top level of a module declares a mutable binding shared across
# all importers. Mutating module-level state makes call order matter, creates
# hidden coupling between unrelated parts of the codebase, and complicates
# testing (state leaks between tests unless explicitly reset).
#
# Note: `const` prevents reassignment but does not prevent mutation of objects
# and arrays — a `const` holding an object is still mutable state.
#
# ## How to fix
#
# - If the value never changes, use `const`.
# - If the value changes only during initialization, use a factory function
#   or class that owns the state.
# - If the value changes at runtime, make the mutation explicit by passing it
#   as a parameter or wrapping it in a store/context object.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Simple module-level caches
# (e.g., `let cache = null`) are idiomatic and acceptable in many codebases.

; Detects: let declarations at the top level of a module
(program
  (lexical_declaration
    "let"
    (variable_declarator
      name: (identifier) @match)))
