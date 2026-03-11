# ---
# id = "typescript/no-inferrable-types"
# severity = "info"
# tags = ["style", "redundancy"]
# message = "Type annotation is redundant — TypeScript infers this type from the literal"
# languages = ["typescript", "tsx"]
# enabled = false
# ---
#
# TypeScript can infer the type of a variable from its initialiser. When
# the initialiser is a string, number, or boolean literal, adding an explicit
# type annotation (`const x: string = "hello"`) is redundant: the compiler
# already knows the type without being told.
#
# Redundant annotations add noise without adding information, and they must
# be kept in sync manually if the initialiser changes.
#
# ```typescript
# // Redundant — TypeScript infers `string` from "hello":
# const name: string = "hello";
#
# // Clean — type is obvious from the value:
# const name = "hello";
# ```
#
# ## How to fix
#
# Remove the type annotation:
# ```typescript
# // Before:
# const count: number = 0;
# const flag: boolean = true;
#
# // After:
# const count = 0;
# const flag = true;
# ```
#
# ## When to disable
#
# This rule is at info severity and is disabled by default. Explicit
# annotations can be useful as documentation in public APIs or when
# you want to ensure a variable is widened to a base type rather than
# narrowed to a literal type. Disable per file if annotations-as-docs
# is a team convention.

; const/let x: string = "literal" — string type inferred from literal
(variable_declarator
  type: (type_annotation
    (predefined_type) @_type)
  value: (string)
  (#eq? @_type "string")) @match

; const/let x: number = 42 — number type inferred from literal
(variable_declarator
  type: (type_annotation
    (predefined_type) @_type)
  value: (number)
  (#eq? @_type "number")) @match

; const/let x: boolean = true — boolean type inferred from literal
(variable_declarator
  type: (type_annotation
    (predefined_type) @_type)
  value: (true)
  (#eq? @_type "boolean")) @match

; const/let x: boolean = false — boolean type inferred from literal
(variable_declarator
  type: (type_annotation
    (predefined_type) @_type)
  value: (false)
  (#eq? @_type "boolean")) @match
