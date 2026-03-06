# normalize-syntax-rules/src/builtin

Embedded builtin rule files, compiled into the binary via `include_str!`.

`mod.rs` declares the `BUILTIN_RULES` constant (a `&[BuiltinRule]` slice) that references all 36 `.scm` rule files. Rules are organized by language namespace: Rust (11 rules: `todo-macro`, `println-debug`, `dbg-macro`, `expect-empty`, `unwrap-in-impl`, `unnecessary-let`, `unnecessary-type-alias`, `chained-if-let`, `numeric-type-annotation`, `tuple-return`, `static-mut`), JavaScript (5: `console-log`, `unnecessary-const`, `module-let`, `var-declaration`, `typeof-string`), TypeScript (1: `tuple-return`), Python (6: `print-debug`, `breakpoint`, `tuple-return`, `module-assign`, `bare-except`, `mutable-default-arg`), Go (4: `fmt-print`, `many-returns`, `package-var`, `error-ignored`), Ruby (4: `binding-pry`, `global-var`, `rescue-exception`, `puts-in-lib`), and cross-language (3: `no-todo-comment`, `no-fixme-comment`, `hardcoded-secret`).
