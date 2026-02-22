# ---
# id = "rust/dbg-macro"
# severity = "warning"
# tags = ["debug-print", "cleanup"]
# message = "dbg!() macro found - remove before committing"
# languages = ["rust"]
# allow = ["**/tests/**"]
# ---

((macro_invocation
  macro: (identifier) @_name
  (#eq? @_name "dbg")) @match)
