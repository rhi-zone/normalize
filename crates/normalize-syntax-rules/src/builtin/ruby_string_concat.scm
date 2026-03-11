# ---
# id = "ruby/string-concat"
# severity = "warning"
# tags = ["style", "readability"]
# message = "Prefer string interpolation `\"#{expr}\"` over string concatenation with `+`"
# languages = ["ruby"]
# enabled = false
# ---
#
# Ruby provides string interpolation as a first-class feature: `"Hello, #{name}!"`.
# Using `+` to concatenate strings is idiomatic in many languages, but in Ruby
# it is generally discouraged because:
#
# - Interpolation is more readable: all parts of the string are visible in one
#   expression without the `+ " " +` noise.
# - Interpolation does not require explicit `.to_s` calls; non-string values
#   are coerced automatically via `#to_s`.
# - `+` allocates an intermediate string object for each concatenation;
#   interpolation builds the final string in one step.
#
# ```ruby
# # Less idiomatic:
# greeting = "Hello, " + name + "!"
#
# # More idiomatic:
# greeting = "Hello, #{name}!"
# ```
#
# ## How to fix
#
# Collapse the concatenation into a single interpolated string literal.
# Replace each `+ expr +` boundary with `#{expr}` inside the string.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Frozen string literals,
# repeated appends to a buffer (`<<`), or concatenation for non-string
# types are not flagged by this rule. Disable per-file if the codebase
# uses concatenation consistently.

; String concatenation: left operand is a string literal
(binary
  left: (string)
  operator: "+"
  right: _) @match
