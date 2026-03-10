# ---
# id = "typescript/no-any"
# severity = "warning"
# tags = ["style", "correctness"]
# message = "Explicit `any` type disables type checking for this binding"
# languages = ["typescript", "tsx"]
# enabled = false
# ---
#
# TypeScript's `any` type is an escape hatch that opts a value out of the
# type system entirely. Assignments to `any` are unchecked, property
# accesses on `any` are unchecked, and `any` propagates through
# expressions — any computation derived from `any` is also `any`.
#
# Using `any` in a type annotation defeats the purpose of TypeScript:
# the compiler cannot catch type errors at that boundary, and IDEs lose
# completion and documentation. Bugs that TypeScript would catch
# statically must be found at runtime instead.
#
# ## How to fix
#
# Replace `any` with the actual type, `unknown` (safe supertype that
# requires narrowing before use), or a generic type parameter:
#
# ```typescript
# // Bad:  function parse(input: any): any
# // Good: function parse(input: unknown): ParseResult
# // Good: function identity<T>(x: T): T
# ```
#
# For third-party code without types, use `unknown` and narrow with
# type guards, or add/improve the `@types/` package.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Migration codebases
# that are incrementally adding types may need to allow `any` temporarily.
# Use the allow list to exclude specific files or directories.

; Type annotations containing `any`
((predefined_type) @match
  (#eq? @match "any"))
