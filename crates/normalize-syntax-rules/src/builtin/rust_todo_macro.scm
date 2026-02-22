# ---
# id = "rust/todo-macro"
# severity = "warning"
# tags = ["cleanup"]
# message = "todo!() macro found - implement before merging"
# languages = ["rust"]
# allow = ["**/tests/**", "**/*_test.rs", "**/test_*.rs"]
# ---
#
# `todo!()` and `unimplemented!()` panic at runtime when the code path is
# reached. Merging them says "this feature is incomplete and will crash
# users." Unlike a compile error, they are invisible until someone hits the
# branch in production.
#
# ## How to fix
#
# Implement the missing logic before merging. If the feature is gated behind
# a flag, return an appropriate error instead of panicking.
#
# ## When to disable
#
# Feature branches where stub implementations are intentional and the branch
# is not being merged to a release target. Add the file to the allow list
# for the duration.

((macro_invocation
  macro: (identifier) @_name
  (#any-of? @_name "todo" "unimplemented")) @match)
