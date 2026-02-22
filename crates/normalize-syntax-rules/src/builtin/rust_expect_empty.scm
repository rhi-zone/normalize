# ---
# id = "rust/expect-empty"
# severity = "warning"
# tags = ["error-handling"]
# message = ".expect() with empty string - provide context message"
# languages = ["rust"]
# ---
#
# `.expect("")` with an empty message provides no diagnostic value when it
# panics — you see "called Result::unwrap() on an Err value" but nothing
# about what failed or why. The only advantage `.expect()` has over
# `.unwrap()` is the ability to add context.
#
# ## How to fix
#
# Provide a message that describes what the program expected to be true at
# this point, e.g., `.expect("config file must exist at startup")`. The
# message should read naturally as "X should be true here."
#
# ## When to disable
#
# Rarely justified. An empty expect message is strictly worse than a
# descriptive one.

; Detects: .expect("") with empty string literal
((call_expression
  function: (field_expression
    field: (field_identifier) @_method)
  arguments: (arguments
    (string_literal) @_msg)
  (#eq? @_method "expect")
  (#eq? @_msg "\"\"")) @match)
