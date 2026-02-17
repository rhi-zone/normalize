# ---
# id = "rust/chained-if-let"
# severity = "info"
# message = "Nested if-let can be chained with && in Rust 2024+"
# languages = ["rust"]
# requires = { "rust.edition" = ">=2024" }
# allow = ["**/tests/**"]
# enabled = false
# ---

; Match if-let containing another if-let as sole block content
((if_expression
  condition: (let_condition)
  consequence: (block
    (expression_statement
      (if_expression
        condition: (let_condition))))) @match)
