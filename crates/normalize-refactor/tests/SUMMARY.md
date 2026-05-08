# normalize-refactor/tests

Integration tests for normalize-refactor.

- `cross_file.rs` — tests for all ModuleResolver implementations: workspace_config, module_of_file, resolve; uses fixture directories under `xfile/`; includes `module_resolver_coverage_matrix` asserting resolver presence for all GP languages
- `fixtures/` — test fixture files (source code snippets used as inputs)
