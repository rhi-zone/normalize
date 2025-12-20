# Moss Roadmap

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

## Next Up

1. Fix CI test failures (Symbol.to_dict, RAGAPI constructor, vector store metadata)
2. Review/refactor vector store API for consistency

## Active Backlog

**CI Failures (Dec 2025):**
- [ ] `Symbol.to_dict` missing in CLI JSON output (test_cli.py skeleton/context)
- [ ] `ControlFlowGraph.entry` attribute missing (test_cli.py cfg)
- [ ] `Export.export_type` attribute missing (test_cli.py deps)
- [ ] `RAGAPI()` constructor signature mismatch (test_rag_integration.py)
- [ ] Vector store metadata validation errors (test_vector_store.py)

**Small:**
- [ ] Multiple agents concurrently - no requirement to join back to main stream
- [ ] Graceful failure - handle errors without crashing, provide useful feedback
- [ ] MCP response ephemeral handling - large responses should stream/page instead of filling context
- [ ] Agent sandboxing - restrict bash/shell access, security-conscious CLI wrappers

**Medium:**
- [ ] Study Goose's context revision (`crates/goose/src/`)
- [ ] Port `context` command to Rust (if context extraction becomes hot path)
- [ ] Port `overview` command to Rust (fast codebase overview)

**Large:**
- [ ] Sessions as first-class - resumable, observable work units

## Future Work

### Skills System
- [ ] `TriggerMode` protocol for plugin-extensible triggers
- [ ] `.moss/skills/` directory for user-defined skills
- [ ] Trigger modes: constant, rag, directory, file_pattern, context

### MCP & Protocols
- [ ] Extension validation before activation
- [ ] Permission scoping for MCP servers
- [ ] A2A protocol integration

### Online Integrations
- [ ] GitHub, GitLab, Forgejo/Gitea - issues, PRs, CI
- [ ] Trello, Jira, Linear - task management
- [ ] Bidirectional sync with issue trackers

### Code Quality
- [ ] `moss patterns` - detect architectural patterns
- [ ] `moss refactor` - detect opportunities, apply with rope/libcst
- [ ] `moss review` - PR analysis using rules + LLM

### LLM-Assisted Operations
- [ ] `moss gen-tests` - generate tests for uncovered code
- [ ] `moss document` - generate/update docstrings
- [ ] `moss explain <symbol>` - explain any code construct
- [ ] `moss localize <test>` - find buggy code from failing test

### Agent Infrastructure
- [ ] Architect/Editor split - separate reasoning from editing
- [ ] Configurable agent roles in `.moss/agents/`
- [ ] Multi-subtree parallelism for independent work
- [ ] Terminal subagent with persistent shell session

### Evaluation
- [ ] SWE-bench harness - benchmark against standard tasks
- [ ] Anchor patching comparison vs search/replace vs diff
- [ ] Skeleton value measurement - does structural context help?

### Reference Resolution (GitHub-level)
- [ ] Full import graph with alias tracking (`from x import y as z`)
- [ ] Variable scoping analysis (what does `x` refer to in context?)
- [ ] Type inference for method calls (`foo.bar()` where `foo: Foo`)
- [ ] Cross-language reference tracking (Python ↔ Rust)

## Deferred

- Log format adapters - after loop work validates architecture

## Notes

### Key Findings
- **86.9% token reduction** using skeleton vs full file (dwim.py: 3,890 vs 29,748 chars)
- **12x output token reduction** with terse prompts (1421 → 112 tokens)
- **90.2% token savings** in composable loops E2E tests
- **93% token reduction** in tool definitions using compact encoding (8K → 558 tokens)

### Performance Profiling (Dec 2025)

**Rust CLI (indexed, warmed):**
- Fast (3-14ms): path, tree --depth 2, search-tree, callers, expand, grep
- Medium (40-46ms): symbols, skeleton, callees, complexity, deps, anchors
- Slow (66ms): summarize (tree-sitter full parse)
- Slowest (95ms): health (parallel codebase scan, 3561 files)

**Python API (with Rust CLI):**
- skeleton: 53ms (single file tree-sitter)
- find_symbols: ~1ms via Rust CLI (was 723ms with Python scan)
- grep: ~4ms with Rust CLI

**Completed Optimizations:**
1. ✅ Rust CLI grep with ripgrep - 9.7s → 4ms (2400x speedup)
2. ✅ Rust health with rayon - 500ms → 95ms (5x speedup)
3. ✅ Rust CLI find-symbols with indexed SQLite - 723ms → 1ms (720x speedup)

### Dogfooding Observations (Dec 2025)
- `skeleton_format` / `skeleton_expand` - very useful, genuinely saves tokens
- `complexity_get_high_risk` - instant actionable data in one call
- `search_find_symbols` - now recursively finds methods inside classes (fixed Dec 2025)
- `explain_symbol` - shows callers/callees for a function (added Dec 2025)
- `guessability_score` - evaluate codebase structure quality

**Missing/wanted:**
- `search_by_keyword` - semantic search across functions, not just name matching

**Recently added:**
- `search_find_related_files` - files that import/are imported by a given file
- `search_summarize_module` - "what does this module do?"

**Friction:**
- Error messages should be specific ("File not found: X" not "No plugin for .py")
- `to_compact()` output feels natural; raw data structures feel clunky

### Agent Lessons
- Don't assume files exist based on class names - `SearchAPI` is in `moss_api.py`, not `search.py`
- Tools aren't perfect - when an error seems wrong, question your inputs before assuming the tool is broken
- Tool design: return specific error messages ("File not found: X") not generic ones ("No plugin for .py")
- **Check before creating**: Always search for existing files before creating new ones (e.g., `prior_art.md` vs `prior-art.md`)
- **Don't read entire large files**: Use grep/skeleton to find relevant section first, then read only what's needed

### Design Principles
See `docs/philosophy.md` for full tenets. Key goals:
- Minimize LLM usage (structural tools first)
- Maximize useful work per token
- Low barrier to entry, works on messy codebases
