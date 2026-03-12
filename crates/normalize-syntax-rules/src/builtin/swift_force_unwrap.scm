# ---
# id = "swift/force-unwrap"
# severity = "warning"
# tags = ["bug-prone", "safety"]
# message = "Force unwrap (!) will crash at runtime if the value is nil"
# languages = ["swift"]
# allow = ["**/test/**", "**/tests/**"]
# enabled = false
# ---
#
# The force unwrap operator (`!`) crashes at runtime with a fatal error
# if the optional is nil. Prefer safe unwrapping with `if let`, `guard let`,
# or the nil-coalescing operator `??`.
#
# ## How to fix
#
# ```swift
# // Before
# let name = user.name!
# // After
# guard let name = user.name else { return }
# // Or
# let name = user.name ?? "Unknown"
# ```
#
# ## When to disable
#
# Disabled by default. Enable if you want to flag force unwraps. Test
# code is already excluded.

((postfix_expression
  (bang)) @match)
