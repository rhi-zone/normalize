# ---
# id = "python/tuple-return"
# severity = "info"
# tags = ["style"]
# message = "Function returns tuple - consider using a dataclass or NamedTuple"
# languages = ["python"]
# enabled = false
# ---
#
# A function annotated to return `tuple[str, int]` forces callers to access
# fields by index (`result[0]`, `result[1]`), which is fragile and opaque.
# Adding a field shifts all existing indices, and there is nothing in the
# type to say what each position means.
#
# ## How to fix
#
# Use `typing.NamedTuple` or `@dataclass` to define a result type with
# named fields. Callers get named access and IDE completions; the type
# appears in tool output with meaningful field names.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Private helper functions
# returning simple `(value, error)` pairs are often not worth a dedicated
# type.

; Detects functions with return type annotation tuple[...]
; NamedTuple or dataclass provides named access and better IDE support
(function_definition
  return_type: (type
    (generic_type
      (identifier) @_tuple
      (#eq? @_tuple "tuple"))) @match)
