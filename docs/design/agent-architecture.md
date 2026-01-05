# Agent Architecture

## Core Model

An "agent" is a **state machine + prompt + tool access**. Not a separate process.

```
┌─────────────────────────────────────────┐
│            State Machine                │
│  (explorer → evaluator → explorer...)   │
├─────────────────────────────────────────┤
│              Prompt/Role                │
│  (explorer | auditor | refactorer | ..) │
├─────────────────────────────────────────┤
│            Tool Access                  │
│  (view, text-search, edit?, shell?)     │
└─────────────────────────────────────────┘
```

## State Machine (shared)

All roles use the same state machine with explorer/evaluator separation:

- **Explorer**: investigates, runs commands, gathers information
- **Evaluator**: judges results, decides to answer or continue

Working memory (`$(keep)`, `$(drop)`, `$(note)`) persists across turns.
Loop detection prevents infinite cycling.

## Roles (prompt variants)

Each role is a prompt that defines:
- What the agent is trying to accomplish
- How it should interpret results
- When it should conclude

### Explorer (default)
- **Purpose**: answer questions about the codebase
- **Tools**: view, text-search, run (read-only shell)
- **Output**: answer with evidence

### Auditor
- **Purpose**: find issues (security, quality, patterns)
- **Tools**: view, text-search, run (read-only shell)
- **Output**: findings with locations and severity

### Refactorer (future)
- **Purpose**: make changes to fix issues
- **Tools**: view, text-search, edit, run (with validation)
- **Output**: applied changes + verification

## Tool Access Levels

| Level | Tools | Use Case |
|-------|-------|----------|
| read-only | view, text-search | exploration, auditing |
| read-shell | + run (no writes) | deeper investigation |
| edit | + edit | refactoring, fixes |
| full | + unrestricted shell | dangerous, needs sandbox |

## Dispatch

Two modes:
1. **Explicit**: `moss @agent --role auditor "find security issues"`
2. **Auto-dispatch**: classifier picks role based on task phrasing

Auto-dispatch is a lightweight LLM call or keyword matching:
- "find", "search", "where", "how" → explorer
- "audit", "check", "issues", "vulnerabilities" → auditor
- "fix", "refactor", "change", "update" → refactorer

## Context Flow

```
User Task
    │
    ▼
┌─────────┐
│Dispatch │ (pick role)
└────┬────┘
     │
     ▼
┌─────────┐     ┌──────────┐
│Explorer │────▶│Evaluator │
└────┬────┘     └────┬─────┘
     │               │
     │◀──────────────┘ (continue)
     │
     ▼
   Answer
```

Dispatcher sees: task description only (no codebase context).
State machine sees: task + tool outputs + working memory.

## Subagents

Agents can spawn other agents for subtasks:
- Refactorer spawns Explorer to understand code before changing
- Auditor spawns Explorer to investigate suspicious patterns

Subagent results flow back as context, not as direct answers.

## Open Questions

1. **Validation loop**: Should refactorer automatically run tests? Or is that a separate "validator" role?

2. **Trust boundaries**: Edit-capable agents need guardrails. Options:
   - Confirmation prompts
   - Shadow worktree (changes in isolation until validated)
   - Restricted paths (only touch files in --only glob)

3. **Context handoff**: When dispatcher routes to a role, what context does the role start with?
   - Just the task?
   - Task + file tree?
   - Task + dispatcher's analysis?

4. **Role composition**: Can a single task use multiple roles sequentially?
   - "Audit for security issues and fix them" = auditor → refactorer pipeline
