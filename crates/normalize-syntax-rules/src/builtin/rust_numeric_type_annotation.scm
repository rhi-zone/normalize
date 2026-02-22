# ---
# id = "rust/numeric-type-annotation"
# severity = "error"
# tags = ["style"]
# message = "Prefer literal suffix over type annotation (e.g., 0.0f32 instead of x: f32 = 0.0)"
# languages = ["rust"]
# enabled = false
# ---
#
# `let x: f32 = 0.0` and `let x = 0.0f32` are equivalent, but the suffix
# form is idiomatic Rust — it keeps type information with the literal,
# avoids a redundant annotation, and reads naturally at the point of use.
# Note: this rule uses error severity in its frontmatter, but mismatches
# between the annotation and literal type are already caught by the compiler;
# this rule targets style, not correctness.
#
# ## How to fix
#
# Move the type to the literal suffix: `let x = 0.0f32` or `let n = 42u64`.
# Remove the `: TypeName` annotation from the binding.
#
# ## When to disable
#
# When the annotation adds clarity in a complex expression context, or when
# the literal appears as part of a larger type inference chain. Disabled by
# default (style preference).

; f32 with type annotation - should use _f32 suffix instead
((let_declaration
  type: (primitive_type) @_type
  value: (float_literal) @_val
  (#eq? @_type "f32")
  (#not-match? @_val "f32$")) @match)

; Non-default integer types with annotation - should use suffix instead
; (i32 is default, so i32 annotation with unsuffixed literal is ok)
((let_declaration
  type: (primitive_type) @_type
  value: (integer_literal) @_val
  (#any-of? @_type "u8" "u16" "u32" "u64" "u128" "usize" "i8" "i16" "i64" "i128" "isize")
  (#not-match? @_val "(u8|u16|u32|u64|u128|usize|i8|i16|i64|i128|isize)$")) @match)

; Negative integers (unary_expression with integer_literal)
((let_declaration
  type: (primitive_type) @_type
  value: (unary_expression
    (integer_literal) @_val)
  (#any-of? @_type "i8" "i16" "i64" "i128" "isize")
  (#not-match? @_val "(i8|i16|i64|i128|isize)$")) @match)
