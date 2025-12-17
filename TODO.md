# Moss Roadmap

See `CHANGELOG.md` for completed features (Phases 15-21, 23-24).

## In Progress

### Phase 22: Synthesis Integration

The synthesis framework scaffolding is complete but **code generation is not implemented**. The framework can decompose problems but returns `pass # TODO` placeholders.

#### 22a: Core Framework ✅
- [x] Directory structure (`src/moss/synthesis/`)
- [x] Abstract interfaces (`Specification`, `Context`, `Subproblem`)
- [x] `DecompositionStrategy` ABC
- [x] `Composer` ABC (SequentialComposer, FunctionComposer, CodeComposer)
- [x] `StrategyRouter` (reuse DWIM TFIDFIndex)
- [x] `SynthesisFramework` (domain-agnostic engine)
- [x] Integration with shadow git, memory, event bus
- [x] Tests (framework tests pass)

#### 22b: Code Synthesis Domain 🚧
- [x] `TypeDrivenDecomposition` strategy
- [x] `TestDrivenDecomposition` strategy
- [x] `PatternBasedDecomposition` strategy
- [ ] **Actual code generation** (currently returns placeholder)
- [ ] `TestExecutorValidator` (run tests, verify solutions)
- [ ] LLM integration for code generation

#### 22c: CLI & Integration 🚧
- [x] `moss synthesize` CLI command
- [x] `--dry-run` and `--show-decomposition` flags
- [ ] Integrate with `moss edit` (fallback for complex tasks)
- [ ] Synthesis configuration presets

#### 22d: Optimization & Learning 🚧
- [x] Caching infrastructure (SynthesisCache, SolutionCache)
- [x] Parallel subproblem solving (asyncio.gather)
- [x] Scale testing (depth 20+ problems)
- [ ] Memory-based strategy learning (interface exists, no implementation)
- [ ] Performance benchmarks (no real code to benchmark)

## Backlog

### Future: Code Generation Backend
Options for implementing actual code generation:
- Claude API integration for semantic synthesis
- Template-based generation for common patterns
- GPT-4 / local model (Ollama) support

### Future: Multi-Language Expansion
- Full TypeScript/JavaScript synthesis support
- Go and Rust synthesis strategies
- Language-agnostic pattern matching

### Future: Enterprise Features
- Team collaboration (shared caches)
- Role-based access control
- Audit logging
