# Hybrid Loops: Combining Multiple Tool Sources

Hybrid loops use `CompositeToolExecutor` to route tool calls to different backends based on prefix. This enables loops that combine local structural tools with external MCP servers and LLM calls.

Note: For declarative workflow definitions, see `normalize workflow` which uses TOML files. This doc covers the low-level `AgentLoop` execution primitives.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    CompositeToolExecutor                     │
├─────────────────────────────────────────────────────────────┤
│  Prefix Routing:                                            │
│    "normalize."  → NormalizeToolExecutor (structural analysis)        │
│    "mcp."   → MCPToolExecutor (external MCP server)         │
│    "llm."   → LLMToolExecutor (LLM generation)              │
│    (no prefix) → default executor                           │
└─────────────────────────────────────────────────────────────┘
```

## Basic Usage

```python
from normalize.agent_loop import (
    AgentLoop,
    AgentLoopRunner,
    CompositeToolExecutor,
    LLMToolExecutor,
    LoopStep,
    NormalizeToolExecutor,
    StepType,
)

# Create individual executors
moss_executor = NormalizeToolExecutor(root=project_root)
llm_executor = LLMToolExecutor(config=llm_config)

# Combine them with prefix routing
composite = CompositeToolExecutor(
    executors={
        "normalize.": moss_executor,
        "llm.": llm_executor,
    },
    default=moss_executor,
)

# Define a loop using prefixed tool names
loop = AgentLoop(
    name="hybrid_analysis",
    steps=[
        LoopStep(name="view_file", tool="normalize.skeleton.format"),
        LoopStep(name="analyze", tool="llm.analyze", input_from="view_file"),
    ],
    exit_conditions=["analyze.success"],
)

# Run
runner = AgentLoopRunner(composite)
result = await runner.run(loop, initial_input)
```

## With External MCP Server

```python
from normalize.agent_loop import MCPServerConfig, MCPToolExecutor

# Configure MCP server
mcp_config = MCPServerConfig(
    command="npx",
    args=["@anthropic/mcp-server-filesystem"],
    cwd="/project",
)

# Create and connect MCP executor
mcp_executor = MCPToolExecutor(mcp_config)
await mcp_executor.connect()

# Add to composite
composite = CompositeToolExecutor(
    executors={
        "normalize.": moss_executor,
        "mcp.": mcp_executor,
        "llm.": llm_executor,
    }
)

# Loop can now use MCP tools
loop = AgentLoop(
    name="with_mcp",
    steps=[
        LoopStep(name="read", tool="mcp.read_file"),
        LoopStep(name="analyze", tool="normalize.skeleton.format", input_from="read"),
    ],
    ...
)

# Cleanup
await mcp_executor.disconnect()
```

## Prefix Stripping

The `CompositeToolExecutor` automatically strips the prefix before passing to the underlying executor:

| Loop Tool Name | Executor | Actual Tool Called |
|----------------|----------|-------------------|
| `normalize.skeleton.format` | NormalizeToolExecutor | `skeleton.format` | <!-- doc-check: ignore -->
| `mcp.read_file` | MCPToolExecutor | `read_file` |
| `llm.analyze` | LLMToolExecutor | `analyze` |

## Available NormalizeAPI Tools

Tools available via `NormalizeToolExecutor` (use with `normalize.` prefix). These are internal Python wrappers that shell out to the Rust CLI. For direct CLI usage, prefer `normalize view`, `normalize edit`, `normalize analyze`.

- `skeleton.format` - Extract file skeleton as text (wraps `normalize view`)
- `skeleton.extract` - Extract skeleton as data structure
- `skeleton.expand` - Get full source of a symbol
- `validation.validate` - Run syntax and linting checks
- `patch.apply` - Apply a patch to a file
- `patch.apply_with_fallback` - Apply patch with text fallback
- `anchor.find` / `anchor.resolve` - Find code anchors
- `complexity.analyze` - Analyze code complexity

Note: The `skeleton.*` tools predate the Rust rewrite. They're subprocess wrappers that should be simplified (see TODO.md).

## Exit Conditions

Exit conditions use the format `{step_name}.success`. The loop exits successfully when the named step completes:

```python
loop = AgentLoop(
    name="example",
    steps=[...],
    exit_conditions=["final_step.success"],  # Exit when final_step succeeds
)
```

If no exit conditions are specified, the loop exits after all steps complete.

## Best Practices

1. **Use meaningful prefixes**: `normalize.`, `mcp.`, `llm.` clearly indicate the tool source
2. **Set a default executor**: Handle unprefixed tools gracefully
3. **Connect MCP before running**: MCP executors require async connection
4. **Disconnect on cleanup**: Always disconnect MCP executors when done
5. **Use mock mode for testing**: Set `LLMConfig(mock=True)` for development

## Example: Full Hybrid Loop

See `examples/hybrid_loop.py` for a complete working example.
