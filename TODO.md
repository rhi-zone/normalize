# Moss Roadmap

## Backlog

### Phase 22: Synthesis Integration (from prose)

See `~/git/prose/moss/` for detailed design documents.

#### Phase 22a: Core Synthesis Framework
- [ ] Create directory structure (`src/moss/synthesis/`)
- [ ] Implement abstract interfaces (`Specification`, `Context`, `Subproblem`)
- [ ] Implement `DecompositionStrategy` ABC
- [ ] Implement `Composer` ABC
- [ ] Implement `StrategyRouter` (reuse DWIM TFIDFIndex)
- [ ] Implement `SynthesisFramework` (domain-agnostic engine)
- [ ] Integration with shadow git, memory, event bus
- [ ] Tests for core framework (>80% coverage)

#### Phase 22b: Code Synthesis Domain
- [ ] Create domain structure (`src/moss/domains/synthesis/`)
- [ ] Implement `TypeDrivenDecomposition` strategy
- [ ] Implement `TestDrivenDecomposition` strategy
- [ ] Implement `TestExecutorValidator`
- [ ] Implement `CodeComposer`
- [ ] Integration tests for code synthesis

#### Phase 22c: CLI & Integration
- [ ] Add `moss synthesize` CLI command
- [ ] Integrate with `moss edit` (fallback for complex tasks)
- [ ] Add synthesis configuration presets
- [ ] Event emission for progress tracking
- [ ] User documentation

#### Phase 22d: Optimization & Learning
- [ ] Test execution caching
- [ ] Parallel subproblem solving
- [ ] Memory-based strategy learning
- [ ] Scale testing (depth 20+ problems)
- [ ] Performance benchmarks

#### Phase 22e: Polish & Documentation
- [ ] User guide (`docs/synthesis-guide.md`)
- [ ] Strategy guide (`docs/synthesis-strategies.md`)
- [ ] Example gallery (`examples/synthesis/`)
- [ ] API documentation

---

## Completed

### Export & Integration
- VS Code extension (`editors/vscode/`)


### Phase 21: Developer Experience & CI/CD
- Watch mode for tests (auto-run on file changes)
- Metrics dashboard (HTML report of codebase health)
- Custom analysis rules (user-defined patterns)
- Pre-commit hook integration
- Diff analysis (analyze changes between commits)
- PR review helper (summarize changes, detect issues)
- SARIF output (for CI/CD integration)
- GitHub Actions integration

See `docs/phase19-features.md` for detailed documentation.

### Phase 20: Integration & Polish
- CLI improvements: global flags, consistent output module
- Interactive shell (moss shell)
- Performance: caching layer, parallel file analysis
- Configuration: moss.toml, per-directory overrides

### Phase 19: Advanced Features
- **19j**: Configurable Output Verbosity
- **19i**: Multi-file Refactoring
- **19h**: Progress Indicators
- **19g**: Live CFG Rendering
- **19f**: LSP Integration
- **19e**: Visual CFG Output
- **19c**: Auto-fix System
- **19b**: Embedding-based Search
- **19a**: Non-Code Content Plugins

### Earlier Phases
- **Phase 18**: Plugin Architecture
- **Phase 17**: Introspection Improvements
- **Phase 16**: DWIM Semantic Routing
- **Phase 15**: LLM Introspection Tooling
