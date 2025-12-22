# Architecture Review - Dec 22, 2024

Session reviewing TODO items led to uncovering significant architectural debt. This document captures findings and learnings.

## Findings

### Redundant Layers

**SkeletonAPI wraps rust_shim wraps Rust CLI:**
```
MossToolExecutor("skeleton.format")
    → api.skeleton.format()
        → rust_shim.rust_skeleton()
            → subprocess Rust CLI
                → SkeletonExtractor
```
Four layers to do what `subprocess.run(["moss", "view", path])` would do.

**Fix:** Call `rust_shim.passthrough()` directly. Remove Python wrapper methods.

### Two Agent Implementations

| Implementation | Purpose | Lines |
|----------------|---------|-------|
| `DWIMLoop` | DWIM-style agent with task tree, cache | 1151 |
| `AgentLoop` | Generic step executor for workflows | 2744 |

`--vanilla` flag switches between them. Same CLI, different engines.

**Problem:** `AgentLoop` isn't an agent - it's a step executor. `DWIMLoop` bakes in choices that should be composable.

### Three Edit Commands

| Command | What it does |
|---------|--------------|
| Rust `moss edit --replace` | Tree-sitter structural ops |
| Python `moss edit --method structural` | Incomplete, only handles rename |
| Python `moss edit --method synthesis` | LLM-based |

Same name, different behavior.

### DWIMLoop: Right Goals, Wrong Structure

DWIMLoop implements "minimize LLM usage" correctly:
- EphemeralCache: preview + ID instead of full content
- TaskTree: hierarchical context tracking
- Adaptive previews: adjust based on task type

But these are baked in, not composable. Workflows should pick strategies:
```toml
[workflow.strategies]
context = "task_tree"    # or task_list, flat, none
cache = "ephemeral"      # or persistent, none
retry = "exponential"    # or fixed, none
```

**Open question:** How do strategies nest? Sub-steps may want different context than parent. TOML may be too rigid.

## Target Architecture

```
Rust CLI:
  moss view <path>              # View file/symbol
  moss edit <path> --replace    # Structural edit
  moss analyze <path>           # Analysis

Python CLI:
  moss run <workflow>           # Predefined steps (deterministic)
  moss agent <task>             # LLM-driven (dynamic)

Shared:
  rust_shim.passthrough()       # Call Rust CLI
  Composable strategies         # Context, cache, retry
```

Remove:
- Python `edit` (use agent)
- SkeletonAPI wrappers (use rust_shim)
- Parallel implementations (unify or clearly separate)

## Session Learnings

### Process

1. **Docs lie, code doesn't** - Always verify docs against actual implementation
2. **"Why?" is the most important question** - Led to understanding structural issues
3. **Stop and design** - We documented debt, didn't fix it. That's correct.

### Design

4. **Composability > hiding** - Don't hide complexity, make it composable
5. **Naming debt is real** - `edit` means 3 things, `AgentLoop` isn't an agent
6. **Redundancy accumulates fast** - Week-old project, already 3+ layers deep

### Working Style

7. **Propose solutions slowly** - Got pushed back repeatedly for half-baked ideas
8. **Check the philosophy** - Forgot "minimize LLM usage", proposed opposite
9. **Question assumptions** - "Is DWIMLoop overcomplicated?" → Yes, structurally

## Next Steps

See TODO.md "Architecture Cleanup (High Priority)" section.

Key decisions needed:
1. How do strategies compose/nest?
2. Code vs config for complex workflows?
3. Clear Rust/Python boundary definition

## Progress (Dec 22 continued)

Created `src/moss/execution/__init__.py` (~450 lines) with composable primitives:

| Strategy Type | Implementations |
|---------------|-----------------|
| Context | FlatContext, TaskListContext, TaskTreeContext |
| Cache | NoCache, InMemoryCache |
| Retry | NoRetry, FixedRetry, ExponentialRetry |
| LLM | NoLLM, SimpleLLM |

**End goal**: DWIMLoop becomes a predefined workflow, not a special class:

```python
DWIM_WORKFLOW = {
    "context": TaskTreeContext,
    "cache": InMemoryCache,
    "retry": ExponentialRetry(max_attempts=3),
    "llm": SimpleLLM(system_prompt=DWIM_PROMPT),
}
result = agent_loop("task", **DWIM_WORKFLOW)
```

This reduces 1151 lines to ~50 lines of workflow configuration.
