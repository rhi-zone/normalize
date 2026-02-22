# ---
# id = "rust/tuple-return"
# severity = "info"
# tags = ["style"]
# message = "Function returns tuple - consider using a struct with named fields"
# languages = ["rust"]
# enabled = false
# ---
#
# Functions that return tuple types like `(String, usize)` require callers
# to access fields by position (`.0`, `.1`), which is fragile and
# unreadable. Adding a third field shifts all existing access, and there is
# no way to know what each position means without reading the function body.
#
# ## How to fix
#
# Define a struct with named fields for the return type. Named fields make
# call sites self-documenting and allow adding or reordering fields without
# breaking callers.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Small, obvious pairs
# like `(key, value)` or `(start, end)` are idiomatic in Rust and may not
# warrant a struct.

; Detects functions returning tuple types like (A, B)
; Named structs are more self-documenting and refactor-friendly
(function_item
  return_type: (tuple_type) @match)
