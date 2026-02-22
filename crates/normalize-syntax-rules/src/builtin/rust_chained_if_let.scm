# ---
# id = "rust/chained-if-let"
# severity = "info"
# tags = ["style"]
# message = "Nested if-let can be chained with && in Rust 2024+"
# languages = ["rust"]
# requires = { "rust.edition" = ">=2024" }
# allow = ["**/tests/**"]
# enabled = false
# ---
#
# Rust 2024 edition supports chaining `if let` conditions with `&&` in a
# single `if let A = x && let B = y` expression. Nested `if let` inside
# the consequence block achieves the same result but adds an extra
# indentation level and makes the combined condition harder to see at a
# glance.
#
# ## How to fix
#
# Combine the conditions: `if let A = x && let B = y { ... }`. This
# flattens the nesting and makes both conditions visible on the same line.
#
# ## When to disable
#
# This rule requires Rust 2024 edition (configured via `requires` in the
# frontmatter). It is disabled by default — enable it for 2024+ edition
# codebases.

; Match if-let containing another if-let as sole block content
((if_expression
  condition: (let_condition)
  consequence: (block
    (expression_statement
      (if_expression
        condition: (let_condition))))) @match)
