# Moss Roadmap

See `CHANGELOG.md` for completed features (Phases 15-21, 23-25).

See `~/git/prose/moss/` for full synthesis design documents.

## In Progress

### Phase 22: Synthesis Integration

The synthesis framework is operational with a plugin architecture for generators, validators, and libraries.

#### 22a: Core Framework ✅
- [x] Directory structure (`src/moss/synthesis/`)
- [x] Abstract interfaces (`Specification`, `Context`, `Subproblem`)
- [x] `DecompositionStrategy` ABC
- [x] `Composer` ABC (SequentialComposer, FunctionComposer, CodeComposer)
- [x] `StrategyRouter` (TF-IDF-based)
- [x] `SynthesisFramework` engine
- [x] Integration points for shadow git, memory, event bus
- [x] Tests for framework structure

#### 22b: Code Synthesis Domain ✅
- [x] `TypeDrivenDecomposition` - decomposes by type signature
- [x] `TestDrivenDecomposition` - analyzes tests for subproblems
- [x] `PatternBasedDecomposition` - recognizes CRUD/validation patterns
- [x] **`TestValidator`** - run pytest/jest to validate code
- [x] **`TypeValidator`** - mypy/pyright type checking
- [x] **Code generators** - PlaceholderGenerator, TemplateGenerator
- [x] **Validation retry loop** - compose, validate, fix, repeat

#### 22c: CLI & Integration 🚧
- [x] `moss synthesize` CLI command (shows decomposition)
- [x] `--dry-run` and `--show-decomposition` flags
- [ ] **`moss edit` integration** - fallback for complex tasks
  - Design: `~/git/prose/moss/code-synthesis-domain.md` lines 462-521
- [ ] Synthesis configuration presets (default/research/production)

#### 22d: Optimization & Learning 🚧
- [x] Caching infrastructure
- [x] Parallel subproblem solving (asyncio.gather)
- [x] Scale testing structure
- [ ] Memory-based strategy learning (record_outcome exists, no learning)
- [ ] Performance benchmarks

### Phase 25: Synthesis Plugin Architecture ✅

Plugin system for synthesis components inspired by Synquid, miniKanren, DreamCoder, and λ².

#### 25a: Plugin Protocols ✅
- [x] `CodeGenerator` protocol - pluggable code generation
- [x] `SynthesisValidator` protocol - pluggable validation
- [x] `LibraryPlugin` protocol - DreamCoder-style abstraction management
- [x] Metadata types for all plugins

#### 25b: Built-in Plugins ✅
- [x] `PlaceholderGenerator` - fallback TODO generation
- [x] `TemplateGenerator` - user-configurable templates (CRUD, validation, etc.)
- [x] `TestValidator` - pytest/jest test execution
- [x] `TypeValidator` - mypy/pyright type checking
- [x] `MemoryLibrary` - in-memory abstraction storage

#### 25c: Registry & Discovery ✅
- [x] `SynthesisRegistry` with sub-registries
- [x] Entry point discovery (`moss.synthesis.generators`, etc.)
- [x] Global registry with lazy initialization
- [x] 31 tests passing

#### 25d: Framework Integration ✅
- [x] `_solve_atomic()` uses generator plugins
- [x] `_validate_with_retry()` implements retry loop
- [x] Library plugin for abstraction lookup

## Future Work

### Phase D: Strategy Auto-Discovery (TODO)
- [ ] Convert DecompositionStrategy to StrategyPlugin protocol
- [ ] Entry point discovery for strategies
- [ ] Config-based enable/disable

### Phase F: Configuration System (TODO)
- [ ] `[synthesis.*]` sections in moss.toml
- [ ] Template directory configuration
- [ ] Plugin enable/disable
- [ ] Validation retry settings

### Future: LLM Integration
- [ ] `LLMGenerator` - Claude/GPT code generation
- [ ] Streaming generation support
- [ ] Cost estimation and budgeting

### Future: Advanced Library Learning
- [ ] Frequency-based abstraction learning
- [ ] DreamCoder-style compression-based learning
- [ ] Persistent library storage

### Future: Multi-Language Expansion
- Full TypeScript/JavaScript synthesis support
- Go and Rust synthesis strategies

### Future: Enterprise Features
- Team collaboration (shared caches)
- Role-based access control
