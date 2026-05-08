# Roadmap: 0.4 — Cross-file Name Resolution (Phase 0)

*Written: 2026-05-09. Reference doc for future sessions.*

## Goal

JetBrains-parity for cross-file analysis: find-references, rename, and call-graph
that work across file boundaries without LSPs.

## JetBrains parity table

| Feature | JetBrains | normalize 0.3 | normalize 0.4 target |
|---------|-----------|---------------|----------------------|
| Find references (same file) | ✓ | ✓ (locals.scm) | ✓ |
| Find references (cross-file) | ✓ | ✗ | Phase 0–1 |
| Rename (same file) | ✓ | ✓ | ✓ |
| Rename (cross-file) | ✓ | ✗ | Phase 1 |
| Call graph (within file) | ✓ | ✓ (call facts) | ✓ |
| Call graph (cross-file) | ✓ | partial | Phase 0–1 |
| Import resolution | ✓ | partial | Phase 0 |
| Go-to-definition | ✓ | ✗ | Phase 2 |
| Unused exports | ✓ | ✗ | Phase 1 |
| Extract function | ✓ | broken | Phase 3 |

## Phase 0 — Scaffold and Rust resolver

**Commits:**
1. **Scaffold** — Datalog predicates, `ModuleResolver` trait, `normalize-module-resolve` crate, `resolution.dl` rules
2. **Rust resolver** — `RustModuleResolver`: workspace_config (Cargo.toml), module_of_file, resolve; cross-file fixture + tests
3. **Pipeline wiring** — wire resolvers into `normalize structure rebuild`, populate `resolved_import`/`module`/`export` facts

**New predicates (in `normalize-facts-rules-api::relations`):**
- `resolved_import(from_file, to_file, imported_name, local_alias, kind)` — kind ∈ {"direct", "glob", "reexport", "unresolved"}
- `module(file, canonical_module_path)` — canonical module identity of a file
- `export(file, name, kind)` — exported symbols; kind ∈ {"value", "type", "module", "reexport"}
- `reexport(from_file, original_file, original_name, exported_as)` — re-export chains
- `symbol_use(file, name, line)` — symbol reference/use sites
- `resolved_reference(use_file, use_line, def_file, def_name, def_kind)` — resolved symbol references
- `resolved_call(caller_file, caller_name, callee_file, callee_name, line)` — resolved calls
- `module_search_path(workspace_root, language, kind, path)` — search paths for module resolution

**`ModuleResolver` trait (in `normalize-languages::traits`):**
```rust
pub trait ModuleResolver: Send + Sync {
    fn workspace_config(&self, root: &Path) -> ResolverConfig;
    fn module_of_file(&self, root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId>;
    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution;
}
```
Added to `Language` trait as `fn module_resolver(&self) -> Option<&dyn ModuleResolver> { None }`.

**`resolution.dl` rules** (disabled by default, `tags = ["semantic"]`):
- Derives `resolved_reference` from `symbol_use` + `resolved_import` + `symbol`
- Handles direct imports, reexport chains, and local definitions
- Derives `resolved_call` from `call` + `resolved_reference`

## Phase 1 — TypeScript/JavaScript and Python resolvers

- `TsModuleResolver`: tsconfig.json `paths`/`baseUrl`, `node_modules`, relative `./` imports, barrel files
- `PythonModuleResolver`: relative imports (`from .foo import bar`), `__init__.py` packages, sys.path
- Wire `normalize find-references --cross-file <file> <name>`
- Wire `normalize rename --cross-file <file> <old> <new>`

## Phase 2 — Go, Ruby; go-to-definition

- `GoModuleResolver`: go.mod module paths, standard library detection
- `RubyModuleResolver`: `require_relative`, gem structure
- `normalize goto-definition <file> <line>:<col>` → `{file, line, col}`

## Phase 3 — CFG and extract-function

- Control flow graph over `normalize-surface-syntax` IR
- Liveness analysis (Datalog over CFG + resolved names)
- `normalize edit extract-function <file> <range> <name>` with correct return-value detection
- Effect/mutation tracking for parameter inference

## Phase 4 — Type information

- Tiered: unresolved name → resolved definition → inferred type (no full type inference)
- Type info sourced from: explicit annotations (TS, Rust, Python 3.10+), JSDoc, inferred from usage
- Required for: correct parameter types in add-parameter, extract-function with typed return

## Phase 5 — Structured metadata integration

- Semantic analysis outputs (effect bits, type info, liveness flags) stored as structured metadata facts
- Enables: agent queries like "find all functions that mutate X", "find callers that don't check the return value"

## Implementation notes

**Resolution pipeline** (commit 3 wiring):
1. For each indexed file, call `lang.module_resolver()` if present
2. Call `resolver.workspace_config(root)` once per workspace
3. For each file: call `resolver.module_of_file(root, file, cfg)` → populate `module` facts
4. For each import in the index: build `ImportSpec`, call `resolver.resolve(from_file, spec, cfg)`
5. Populate `resolved_import` facts from results
6. Populate `export` facts from visibility + symbol facts

**Conservative by design:** resolvers return `Resolution::NotFound` rather than guessing.
stdlib and third-party crates always return `NotFound` (not `NotApplicable`).
`NotApplicable` is only for languages that have no module system at all (Bash, GLSL, etc.).
