# xfile fixtures

Cross-file fixture workspaces for Phase 0 cross-file name resolution tests.

Subdirectories:
- `rust/` — 3-file Cargo crate fixture for `RustModuleResolver` tests
- `typescript/` — 3-file TypeScript project with `tsconfig.json` for `TsModuleResolver` tests
- `python/` — Python package with relative imports for `PythonModuleResolver` tests
- `go/` — Go module with subpackage for `GoModuleResolver` tests
- `javascript/` — ESM project with `package.json` for `JsModuleResolver` tests
- `ruby/` — two-file Ruby project using `require_relative` for `RubyModuleResolver` tests
