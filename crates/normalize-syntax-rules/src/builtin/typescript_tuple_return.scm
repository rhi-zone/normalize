# ---
# id = "typescript/tuple-return"
# severity = "info"
# message = "Function returns tuple - consider using an interface with named properties"
# languages = ["typescript", "tsx"]
# enabled = false
# ---
#
# TypeScript functions returning tuple types like `[string, number]` require
# callers to use index access (`result[0]`, `result[1]`), which is opaque
# and breaks if the tuple grows. There is no way to know what each position
# represents without reading the return type annotation carefully.
#
# ## How to fix
#
# Define an interface or type alias with named properties and return that
# instead. Callers get descriptive property names and IDE autocompletion,
# and adding fields does not break existing destructuring.
#
# ## When to disable
#
# This rule is disabled by default (info severity). React hooks that return
# `[value, setter]` pairs follow a well-understood convention and are an
# accepted exception.

; Detects functions returning tuple types like [A, B]
; Named interfaces are more self-documenting
(function_declaration
  return_type: (type_annotation (tuple_type)) @match)

(arrow_function
  return_type: (type_annotation (tuple_type)) @match)

(method_definition
  return_type: (type_annotation (tuple_type)) @match)
