# Moss Roadmap

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

## Next Up

1. **Module name DWIM** - Fuzzy matching for file/module names
   - Typo tolerance for common patterns
   - Context-aware suggestions

2. **Complexity hotspots** - Address the 60 functions with complexity ≥15
   - Prioritize by usage frequency
   - Refactor or document complex code

3. **CLI from MossAPI** - Migrate cli.py to generated interface
   - Use introspection to generate CLI commands
   - Reduce duplication between CLI and API

## Active Backlog

**Small:**
- [ ] Model-agnostic naming - don't over-fit to specific LLM conventions
- [ ] Multiple agents concurrently - no requirement to join back to main stream

**Medium:**
- [ ] Study Goose's context revision (`crates/goose/src/`)
- [ ] Agent learning - record mistakes in `.moss/lessons.md`

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

## Deferred

- Log format adapters - after loop work validates architecture

## Notes

### Key Findings
- **86.9% token reduction** using skeleton vs full file (dwim.py: 3,890 vs 29,748 chars)
- **12x output token reduction** with terse prompts (1421 → 112 tokens)
- **90.2% token savings** in composable loops E2E tests

### Dogfooding Observations (Dec 2025)
- `skeleton_format` / `skeleton_expand` - very useful
- New: `search_find_symbols`, `search_grep` - use instead of raw grep/glob
- New: `guessability_score` - evaluate codebase structure quality

### Design Principles
See `docs/philosophy.md` for full tenets. Key goals:
- Minimize LLM usage (structural tools first)
- Maximize useful work per token
- Low barrier to entry, works on messy codebases
