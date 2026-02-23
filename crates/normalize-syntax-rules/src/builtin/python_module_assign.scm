# ---
# id = "python/module-assign"
# severity = "info"
# tags = ["architecture", "mutable-state", "global-state", "mutable-global-state"]
# message = "Module-level assignment - consider using a constant (UPPER_CASE) or encapsulating mutable state"
# languages = ["python"]
# enabled = false
# ---
#
# Module-level assignments to lowercase names introduce mutable global state.
# Unlike `UPPER_CASE` names (the Python convention for constants), a lowercase
# module-level name signals intent to mutate, which makes call order matter,
# creates hidden coupling between importers, and causes state to leak between
# tests unless explicitly reset.
#
# ## How to fix
#
# - If the value never changes, rename to `UPPER_CASE` to signal that intent.
# - If the value is computed once at startup, use a module-level function or
#   class that is called explicitly.
# - If the value changes at runtime, pass it explicitly as a parameter or
#   wrap it in a class that owns the state.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Module-level registries
# and caches (`_cache = {}`) are common and often intentional. Exclude those
# files or add a `normalize-syntax-allow` comment at the declaration site.

; Detects module-level assignments to lowercase names (mutable state).
; Excludes: UPPER_SNAKE_CASE (conventional constants), __dunder__ names (module metadata).
; Note: in tree-sitter-python, assignments are direct children of `module` (no expression_statement wrapper).
(module
  (assignment
    left: (identifier) @match
    (#not-match? @match "^[A-Z][A-Z0-9_]*$")
    (#not-match? @match "^__.*__$")))
