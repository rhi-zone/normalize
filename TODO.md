# Moss Roadmap

## Phase 15: LLM Introspection Tooling

### CLI Enhancements ✅
- [x] Add `--json` output flag to all CLI commands
- [x] `moss skeleton <path>` - Extract and display code skeleton
- [x] `moss anchors <path>` - List all anchors (functions, classes, methods)
- [x] `moss cfg <path> [function]` - Display control flow graph
- [x] `moss deps <path>` - Show dependencies (imports/exports)
- [x] `moss context <path>` - Combined view (skeleton + deps + summary)

### Query Interface ✅
- [x] `moss query` command with pattern matching
- [x] Find functions by signature pattern
- [x] Find classes by inheritance
- [ ] Search by complexity metrics (lines, branches, etc.) - TODO: add line counting

### MCP Server ✅
- [x] Implement MCP server for direct tool access
- [x] Expose skeleton extraction as MCP tool
- [x] Expose anchor finding as MCP tool
- [x] Expose CFG building as MCP tool
- [x] Expose patch application as MCP tool
- [x] Expose dependency extraction as MCP tool
- [x] Expose context generation as MCP tool

### LLM Evaluation ✅
- [x] Use Moss CLI to explore codebases
- [x] Document what works well for LLM consumption (see docs/llm-evaluation.md)
- [x] Identify gaps and iterate

## Phase 16: Plugin Architecture

> **Important**: This phase should only begin AFTER Phase 15 is complete and we've
> validated the current implementation through real-world LLM usage. Premature
> abstraction is worse than no abstraction.

### Plugin System Design
- [ ] Design plugin interface for view providers
- [ ] Implement plugin discovery and loading
- [ ] Create plugin registration and lifecycle management

### Content Type Plugins
- [ ] Refactor Python skeleton extraction as plugin
- [ ] Refactor CFG building as plugin
- [ ] Refactor dependency extraction as plugin
- [ ] Add support for non-code content (markdown, JSON, YAML, etc.)

### Language Plugins
- [ ] TypeScript/JavaScript plugin
- [ ] Go plugin
- [ ] Rust plugin

### Third-Party Extension Support
- [ ] Plugin API documentation
- [ ] Plugin development guide
- [ ] Example plugin implementation

## CI/CD Fixes ✅
- [x] Add ruff to dev dependencies
- [x] Fix CI to install dev extras (`uv sync --extra dev`)
- [x] Remove Python 3.12 from CI (project requires 3.13+)
- [x] Consolidate dev dependencies in pyproject.toml
- [ ] Fix async fixture warnings (pytest 9 deprecation) - may need further work

## Future Ideas
- Real-time file watching and incremental updates
- Language server protocol (LSP) integration
- Visual CFG rendering (graphviz/mermaid output)
- Semantic code search with embeddings
- Multi-file refactoring support
- **Auto-fix for lints**: Design an auto-fix system where lints can suggest and apply
  fixes automatically. This requires careful design work around:
  - Safe vs unsafe fixes
  - Preview/diff before applying
  - Rollback support (integrate with Shadow Git)
  - Conflict resolution when multiple fixes overlap
