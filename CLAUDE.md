# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Moss is a headless agent orchestration layer for AI engineering. It implements a "Compiled Context" approach that prioritizes architectural awareness (AST-based understanding) over raw text processing, with verification loops ensuring correctness before output.

## Development Environment

This project uses Nix flakes for reproducible development environments:

```bash
# Enter dev shell (automatic with direnv, or manual)
nix develop

# Tools available: Python 3.13, uv, ruff, ripgrep
```

## Architecture (from docs/spec.md)

Core components:
- **Event Bus**: Async communication (`UserMessage`, `PlanGenerated`, `ToolCall`, `ValidationFailed`, `ShadowCommit`)
- **Context Host**: Manages View Providers (Skeleton, CFG, Dependency Graph) - delegates to plugins
- **Structural Editor**: AST-based editing with fuzzy anchor matching
- **Policy Engine**: Enforces safety rules (velocity checks, quarantine)
- **Validator**: Domain-specific verification loop (compiler, linter, tests)
- **Shadow Git**: Atomic commits per tool call, rollback via git reset

Data flow: User Request → Config Engine → Planner → Context Host (Views) → Draft → Shadow Git → Validator → (retry loop if error) → Commit Handle

Multi-agent model: Ticket-based (not shared chat history). Agents are isolated microservices passing Handles, not context.

## Conventions

### Commits

Each commit should be a **unit of work** - a single logical change that could be reverted independently. Not "fixed stuff" but "fix: handle null response in validator loop".

### Code Quality

Linting: `ruff check` and `ruff format` (enforced once CI exists)

### Testing Strategy

Tests at all levels:
- **Unit**: Isolated component behavior
- **Integration**: Component interactions (e.g., Context Host ↔ Validator)
- **E2E**: Full flows (user request → commit handle)
- **Fuzzing**: Malformed inputs, edge cases in AST parsing

Run tests before committing. When adding functionality, add corresponding tests.
