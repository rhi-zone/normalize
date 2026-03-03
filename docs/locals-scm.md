# locals.scm Coverage Notes

## Current State
- 65 locals.scm files in `queries/`, 159 fixture tests in `crates/normalize-scope/src/lib.rs`
- All have fixture tests; unverified locals.scm is worse than none

## Languages Done (workspace `queries/` dir)
First session: rust, python, go, java, c, cpp, c-sharp, ruby, bash, kotlin, php, zig, dart,
elixir, erlang, clojure, julia, perl, groovy, d, typescript, javascript, ocaml, scala, lua, r,
tsx, swift, gleam, tlaplus, elm, fsharp, ada, starlark, thrift, objc, nix, rescript, haskell, capnp

Second session: scheme, commonlisp, elisp, prolog, fish, zsh, powershell, vim, sql,
matlab, awk, cmake, typst, hcl, verilog, vhdl, jq, meson, vb, idris, lean, agda, glsl, hlsl, yuri

## Languages Skipped (with reason)
- svelte, vue: injection languages (no scope at that AST level)
- zsh: grammar too limited; `local y=2` → ERROR; simple_expansion is leaf node
- batch: plain SET x=hello → ERROR; function_name and variable_name are UNNAMED nodes
- asm/x86asm: no lexical scoping; labels global; registers architectural
- scss: $variable uses property_name (same as CSS props); can't distinguish without text-matching
- dockerfile: no scope model; all ARG/ENV file-global
- postscript: stack-based; `operator` node is both built-ins and variable refs
- sparql: graph-pattern unification, not lexical scoping
- wit: IDL only; no runtime variable scoping
- Data formats/configs/markups: json, toml, yaml, xml, ini, ron, kdl, asciidoc, markdown,
  html, css, diff, dot, graphql, nginx, caddy, ssh-config, jinja2, textproto, query,
  devicetree, ninja, uiua

## Grammar Quirks by Language

### Scheme / Lisps
- Scheme: flat `list`/`symbol` grammar; all special forms need `#eq?`/`#any-of?` predicates
- CommonLisp: `defun` is specialized; `defun_header` → `defun_keyword` + `sym_lit` (name) + `list_lit` (params)
- Elisp: `function_definition` has named fields `name:` / `parameters:`; `special_form` uses unnamed keyword children

### Functional Languages
- Prolog: `clause_term` scope; all `variable_term` are both def and ref; `_` excluded via `#not-eq?`
- OCaml: scope must be `let_expression` (not `let_binding` which doesn't span `in`-body); curried params use `value_pattern`
- Haskell: `(function name: (variable))` for top-level; `(bind name: (variable))` for `f = ...`; let via `let_in > local_binds > bind name:`
- Idris: `loname` (lowercase) vs `caname` (uppercase); function name = `(funvar . (loname) @def)`; params = `(pat_name (loname) @def)`; refs = `(exp_name (loname) @ref)`
- Agda: type sigs use `function_name` wrapper in lhs (equation clauses don't); `(let (function (lhs (atom ...))))` → Structure error (`lhs>atom` invalid in let-function context per grammar node-types); use `(source_file) @local.scope` not `(module)` (module only covers header)
- Lean: `(def "def" . (identifier) @def)` for name; `(explicit_binder . (identifier) @def)` for params; `(let "let" . (identifier) @def)` for bindings; `(fun (parameters (identifier) @def))` for lambdas
- FSharp: `#set!` predicates harmlessly ignored by engine
- Scala: must scope `function_definition` not just `function_declaration` (arborium missed this)

### Shell / Scripting
- Fish: function name = first `word` after `"function"` keyword; for loop uses `variable_name` node; refs via `variable_expansion > variable_name`
- PowerShell: `function_name` is distinct node; params use `script_parameter > variable`
- Vim: `"let" . (identifier)` anchor captures only LHS; `scoped_identifier` for `l:x`, `s:x`

### C-family
- GLSL/HLSL: nearly identical to C; same `function_declarator > identifier` patterns; HLSL adds `semantics` node ignored
- Yuri (shader): `fn name(params) : return_type { body }` (colon not ->); `function_item` scope (not `block`) so params are visible; `variable_item` for let; no semicolons; refs = `(identifier (symbol) @ref)`

### Build Systems / DSLs
- CMake: `(normal_command . (identifier) @_cmd ... (#eq? @_cmd "set"))` for variable defs; `variable_ref > normal_var > variable` for refs
- Typst: `let` is a NAMED node (not "let" keyword); simple = `(let (ident) @def)`; function = `(let (call (ident) @def))`
- HCL: `attribute (identifier) @def` captures all attrs; avoid duplicate by using only generic pattern
- VHDL: declarations use `identifier`, references use `simple_name` (different node kinds); non-existent node kinds cause query compile failure

### OOP
- VB: `method_declaration name: (identifier)` for Function/Sub; `parameter name:` / `dim_statement name:` / `for_each_statement variable:` have named fields
- ObjC: method_parameter has no `declarator:` field; needs @implementation context

### Database / Query
- SQL: `(cte . (identifier))` anchor for CTE name (no named field); `relation alias:` for table aliases
- Elixir FIXED: custom `#is-match-op!` predicate on named parent capture (unnamed node captures in field position don't get predicates evaluated)

### Misc Quirks
- Nix: arborium intentionally disabled (lazy semantics); node paths needed fixing
- ReScript: uses `value_identifier` not `identifier`
- Top-level code has no scope: defs captured but refs can't resolve without `@local.scope`
- Arborium `(pattern/variable)` and `(expression/variable)` = Neovim field-path syntax, not valid tree-sitter queries
- Dart: no function_declaration node; function_signature + function_body are siblings

## Engine Notes
- `@local.binding-leaf`: declares leaf kinds for recursive destructuring
- `@local.definition.each`: recurses into container collecting binding leaves (used for JS/TS destructuring)
- `#eq?`, `#any-of?`, `#match?` on NAMED captures: work; on UNNAMED captures: don't work
- Workaround for unnamed: use `#is-match-op!` custom predicate on named parent
- `Query::new` fails silently on invalid query → `analyze()` returns None → engine returns empty (safe)
- Structure error = pattern would never match per grammar's node-types.json (even if runtime tree has it)
- xtask `copy_bundled_queries` diffs content; edits to `queries/*.scm` auto-propagate on `cargo xtask build-grammars`
