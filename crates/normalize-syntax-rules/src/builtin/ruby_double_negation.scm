# ---
# id = "ruby/double-negation"
# severity = "warning"
# tags = ["style", "readability"]
# message = "`!!` is used for boolean coercion — prefer explicit `.nil?` check or direct boolean"
# languages = ["ruby"]
# enabled = false
# ---
#
# Ruby's `!!` (double negation) is used to coerce a value to `true` or
# `false`. While technically correct, it is considered poor style because:
#
# - It is obscure: readers unfamiliar with the idiom may not recognise it
#   immediately.
# - There is usually a more expressive alternative: `.nil?`, `.present?`,
#   `.empty?`, or a direct boolean comparison.
# - It adds no safety: `!!nil` is `false`, but so is `nil.nil?`, and the
#   latter communicates intent.
#
# ```ruby
# # Less clear:
# valid = !!value
# active = !!user.active?
#
# # More explicit:
# valid = !value.nil?
# active = user.active? == true
# ```
#
# ## How to fix
#
# Replace `!!expr` with the appropriate explicit check:
# - `!!x` → `!x.nil?` (if you care about nil-ness)
# - `!!x` → `x == true` (if you want strict boolean equality)
# - In boolean context (e.g., an `if` condition), just use `x` directly.
#
# ## When to disable
#
# This rule is disabled by default. Some Ruby codebases use `!!` as a
# conventional boolean cast in method return positions. Disable per file
# if that is an established pattern.

; Double negation `!!expr` — boolean coercion idiom
(unary
  operator: "!"
  operand: (unary
    operator: "!"
    operand: _)) @match
