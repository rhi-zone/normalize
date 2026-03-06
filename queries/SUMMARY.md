# Queries

Workspace-level tree-sitter `locals.scm` query files covering 65 languages. These files define scope, definition, and reference capture nodes used by `normalize-scope`'s `ScopeEngine` for within-file reference resolution. The xtask `build-grammars` command copies these files alongside grammar `.so` libraries; the workspace copies take precedence over any bundled arborium versions. Language coverage and implementation notes are documented in `docs/locals-scm.md`.
