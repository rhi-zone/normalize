# ---
# id = "rust/chained-if-let"
# severity = "error"
# tags = ["style"]
# message = "Nested if-let can be chained with && in Rust 2024+"
# languages = ["rust"]
# requires = { "rust.edition" = ">=2024" }
# allow = ["**/tests/**"]
# enabled = true
# fix = "if $outer_cond && $inner_cond $inner_body"
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
# The auto-fix handles this transformation. Run `cargo fmt` afterwards to
# re-indent the body.
#
# ## When to disable
#
# This rule requires Rust 2024 edition (configured via `requires` in the
# frontmatter). It is disabled by default — enable it for 2024+ edition
# codebases.

; Match if-let containing another if-let as the sole statement in the block,
; where neither if has an else clause.
;
; Anchors (.) enforce no siblings — without them, the pattern would also
; match blocks where the inner if-let is preceded or followed by other
; statements, which cannot be safely chained.
;
; !alternative on both levels:
; - inner: chaining would drop the else branch
; - outer: if the outer has an else, chaining changes when the else fires
;   (original: only when outer condition fails; chained: when either fails)
((if_expression
  condition: (let_condition) @outer_cond
  consequence: (block
    .
    (expression_statement
      (if_expression
        condition: (let_condition) @inner_cond
        consequence: (block) @inner_body
        !alternative))
    .)
  !alternative) @match)
