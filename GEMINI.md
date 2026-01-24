# GEMINI.md

Behavioral rules for Gemini working in this repository.
Design philosophy: `docs/philosophy.md`. Key tenets: Minimize LLM usage, Structure > Text, Library-first.

## Core Rule

ALWAYS NOTE THINGS DOWN. When you discover something important, write it immediately:
- Bugs/issues → fix them or add to TODO.md
- Environment issues → TODO.md
- Design decisions → docs/ or code comments
- Future work → TODO.md
- Conventions → this file

## Negative Constraints

Do not:
- Announce actions with "I will now..." - just do them
- Use markdown formatting in LLM prompts (no bold, headers, code blocks unless required)
- Write preamble or summary in generated content
- Use `os.path` - use `pathlib`
- Catch generic `Exception` - catch specific errors
- Leave work uncommitted
- Write stub code with `pass` or `...` or "TODO: implement" - always provide full implementation
- Skip running tests after code changes
- Assume code works without verification

Our system prompt for sub-agents (`src/normalize/agent_loop.py:LLMConfig.system_prompt`):
"Be terse. No preamble, no summary, no markdown formatting. Plain text only. For analysis: short bullet points, max 5 items, no code."

## Development Environment

Run `uv sync --extra all --extra dev` first. Many features require optional dependencies.

```bash
nix develop                      # Enter dev shell
uv sync --extra all --extra dev  # Install dependencies
```

## Recipes

Scaffold MCP Tool:
1. Add API class in `src/normalize/moss_api.py`
2. Add accessor property to `NormalizeAPI` class
3. Update `src/normalize/gen/introspect.py`: add import and entry to `sub_apis`
4. Run `normalize gen --target=mcp`
5. Reload MCP server

Context Reset (before `/exit`):
1. Commit current work
2. Update TODO.md "Next Up" section
3. Note any open questions

## Dogfooding

**Use normalize CLI for code intelligence** via `uv run normalize`. Returns structure (symbols, skeletons, anchors) instead of raw text, saving ~90% tokens. MCP has historically been non-viable.

Quick reference:
- `uv run normalize skeleton <file>` - understand file structure before reading
- `uv run normalize search <query>` - find function/class definitions
- `uv run normalize complexity` - identify problem areas
- `uv run normalize explain <symbol>` - show callers/callees

Fall back to generic tools (Read/Grep) only for:
- Exact line content needed for editing

## Conventions

### Updating GEMINI.md
Add: workflow patterns, conventions, project-specific knowledge, tool usage patterns.
Don't add: temporary notes (TODO.md), implementation details (docs/), one-off decisions (commit messages).
Keep it slim: If GEMINI.md grows past ~150 lines, refactor content to docs/ and reference it.

### Updating TODO.md
Proactively add features, ideas, patterns, technical debt, integration opportunities.
Keep TODO.md lean (<100 lines). Move completed items to CHANGELOG.md.
- Next Up: 3-5 concrete tasks for immediate work
- Active Backlog: pending items only, no completed
- Future Work: categories with brief items
- To Consolidate: new ideas before proper categorization
- Avoid: verbose descriptions, code examples, duplicate entries

### Working Style

Start by checking TODO.md. Default: work through ALL items in "Next Up" unless user specifies otherwise.
Propose work queue, get confirmation, then work autonomously through all tasks.

Agentic by default - continue through tasks unless:
- Genuinely blocked and need clarification
- Decision has significant irreversible consequences
- User explicitly asked to be consulted

Bail out early if stuck in a loop rather than burning tokens.

Keep sessions fresh. Consider wrapping up when:
- Major feature complete
- 50+ tool calls
- Re-reading files (sign of forgetting)
- Conversation drifted across unrelated topics

Write while researching, not after. Queue review items in TODO.md, don't block for them.

Session handoffs: Add "Next Up" section to TODO.md with 3-5 tasks. Goal is to complete ALL of them next session.

### Commits

Commit consistently. Each commit = one logical change.
Move completed TODOs to CHANGELOG.md.
Avoid using backticks inside double-quoted strings in commit messages to prevent shell escaping issues.

### Code Quality

Linting: `ruff check` and `ruff format`
Tests: Run before committing. Add tests with new functionality.

### Verification Workflow

After writing any code:
1. Run `ruff check` and fix issues
2. Run relevant tests with `uv run pytest <test_file>`
3. If tests fail, fix immediately - do not proceed with broken code
4. Only commit after tests pass

Never claim code is complete without running it or its tests.
