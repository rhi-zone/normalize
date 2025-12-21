# Moss Roadmap

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

## Next Up

- [ ] **Recursive Workflow Learning**: Propose new workflows based on recurrent session patterns
- [ ] **Memory & Resource Metrics**: Show context and RAM usage (with breakdown) for every command
- [ ] **Local Model Constrained Inference**: Implement GBNF (GGML BNF) for structured output
- [ ] **TUI Syntax Highlighting**: High-quality code highlighting in file previews
- [ ] **Adaptive Model Rotation**: Dynamically switch LLM providers based on task latency

## Recently Completed

- **Adaptive model and context control** (Dec 2025)
- **Recursive self-improvement** (Dec 2025)
- **Shadow Git enhancements** (Dec 2025)
- **Advanced TUI capabilities** (Dec 2025)
- **Core architecture & UX** (Dec 2025)

## Active Backlog

- Workflow argument passing improvement
- **Mistake Detection**: Detect when an LLM *maybe* made a mistake (Critic loop enhancement)
- **Shadow Git Access**: Give LLM first-class access to 'Shadow Git' (diffs, rollback, "what did I break?")
- **User Feedback Story**: Improve interruptibility and feedback loops (client-side interrupts, agent "check mail" steps) to handle mid-task corrections.

**Large:**
- [ ] **Comprehensive Telemetry & Analysis**:
  - Track all token usage, patterns, and codebase access patterns by default
  - Store maximal metadata for every session
  - Built-in high-quality analysis tools (CLI & visual)
  - Easy interface for complex custom analysis (e.g. "what files do I edit most with `fix`?")
- [ ] Memory system - layered memory for cross-session learning (see `docs/memory-system.md`)
- [ ] Workflow loader plugin abstraction - extract protocol when Python workflows need it
  - Current: TOML loader is direct implementation
  - Future: `WorkflowLoader` protocol, entry point registration, multiple loader types

### Strict Harness (guardrails for all agents)

**Signal-Only Diagnostics:** (done - see `src/moss/diagnostics.py`, `src/moss/validators.py`)
- [x] Parse `cargo check --message-format=json` instead of raw stderr
- [x] Extract: error code, message, file/line, suggestion - discard ASCII art
- [x] Integrate with validation loop via `DiagnosticValidator`
- [x] "Syntax Repair Engine" system prompt when errors present (see `REPAIR_ENGINE_PROMPT` in `agent_loop.py`)

**Degraded Mode (AST fallback):** (done - see `src/moss/tree_sitter.py`)
- [x] Wrap tree-sitter parse in Result (`ParseResult`)
- [x] On parse failure, fallback to "Text Window" mode (`text_window()`)
- [x] Never block read access due to parse failures

**Peek-First Policy:** (done - see `LoopContext.expanded_symbols` in `agent_loop.py`)
- [x] Constraint: agent cannot edit symbol only seen as skeleton
- [x] Must `expand` before `edit` - enforced in agent loop (`MossToolExecutor.enforce_peek_first`)
- [x] Prevents hallucination of function bodies

**Hunk-Level Rollback:** (done - see `src/moss/shadow_git.py`)
- [x] `DiffHunk` dataclass and `parse_diff()` for diff parsing
- [x] `get_hunks()` - parse branch diff into hunks
- [x] `map_hunks_to_symbols()` - map hunks to AST nodes via tree-sitter
- [x] `rollback_hunks()` - selectively revert specific hunks

## Future Work

### Agent Research & Optimization
- [ ] **LLM Editing Performance Comparison**:
  - Investigate Gemini 3 Flash and Gemini 3 Pro issues with invalid code edits
  - Compare with Claude Code and Opus to identify architectural differences
  - Evaluate if specialized prompting or different edit formats (e.g. diffs) help
- [ ] **YOLO Mode Evaluation**: Evaluate if a "YOLO mode" aligns with Moss architectural principles
- [ ] **Memory Usage Optimization**: Ensure Moss keeps RAM usage extremely low, even for large codebases
- [ ] **Extensible Agent Modes**:
  - Refactor TUI modes (PLAN, READ, WRITE, DIFF, SESSION, BRANCH, SWARM, COMMIT) into a plugin-based system
  - Allow user-defined modes via `.moss/modes/`
- [ ] **'Diffusion-like' methods for large-scale refactors**:
  - Generate contracts/signatures at high levels first
  - Parallelize implementation of components
  - Explore reconciliation strategies for independent components
- [ ] **Small/Local Model Brute Force**: Explore using smaller, faster local models with higher iteration/voting counts
- [ ] **Fine-tuned Tiny Models**:
  - Explore extreme optimization with models like 100M RWKV
  - Benchmark model size vs. reliability
  - High-frontier LLM generated tests for tiny model validation
- [ ] **Pattern detection** - heuristic (frequency, similarity, rapid re-runs) + LLM for judgment
- [ ] **Workflow self-creation** - agent creates workflows from detected patterns autonomously
- [ ] **Workflow discovery** - surface candidates from Makefile/package.json/CI, agent or user picks

### Codebase Tree Consolidation (see `docs/codebase-tree.md`)

**Phase 1: Python CLI delegates to Rust (remove Python implementations)**
- [ ] `skeleton` → delegate to Rust `view`
- [ ] `summarize` → delegate to Rust `view`
- [ ] `anchors` → delegate to Rust `search` with type filter
- [ ] `query` → delegate to Rust `search`
- [ ] `tree` → delegate to Rust `view` (directory-level)
- [x] `context` → delegate to Rust `context` (done)

**Phase 2: Unified tree model**
- [ ] Merge filesystem + AST into single tree data structure
- [ ] Implement zoom levels (directory → file → class → method → params)
- [ ] Consistent "context + node + children" view format

**Phase 3: DWIM integration**
- [ ] Natural language → tree operation mapping
- [ ] "what's in X" → view, "show me Y" → view, "full code of Z" → expand

### Reference Resolution (GitHub-level)
- [ ] Full import graph with alias tracking (`from x import y as z`)
- [ ] Variable scoping analysis (what does `x` refer to in context?)
- [ ] Type inference for method calls (`foo.bar()` where `foo: Foo`)
- [ ] Cross-language reference tracking (Python ↔ Rust)

## Notes

### Key Findings
- **86.9% token reduction** using skeleton vs full file (dwim.py: 3,890 vs 29,748 chars)
- **12x output token reduction** with terse prompts (1421 → 112 tokens)
- **90.2% token savings** in composable loops E2E tests
- **93% token reduction** in tool definitions using compact encoding (8K → 558 tokens)

### Design Principles
See `docs/philosophy.md` for full tenets. Key goals:
- Minimize LLM usage (structural tools first)
- Maximize useful work per token
- Low barrier to entry, works on messy codebases