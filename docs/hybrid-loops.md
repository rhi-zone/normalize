# Hybrid Loops: Combining Multiple Tool Sources

Hybrid loops use `CompositeToolExecutor` to route tool calls to different backends based on prefix. This enables loops that combine local structural tools with external MCP servers and LLM calls.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    CompositeToolExecutor                     │
├─────────────────────────────────────────────────────────────┤
│  Prefix Routing:                                            │
│    "moss."  → MossToolExecutor (structural analysis)        │
│    "mcp."   → MCPToolExecutor (external MCP server)         │
│    "llm."   → LLMToolExecutor (LLM generation)              │
│    (no prefix) → default executor                           │
└─────────────────────────────────────────────────────────────┘
```

## Basic Usage

```python
from moss.agent_loop import (
    AgentLoop,
    AgentLoopRunner,
    CompositeToolExecutor,
    LLMToolExecutor,
    LoopStep,
    MossToolExecutor,
    StepType,
)

# Create individual executors
moss_executor = MossToolExecutor(root=project_root)
llm_executor = LLMToolExecutor(config=llm_config)

# Combine them with prefix routing
composite = CompositeToolExecutor(
    executors={
        "moss.": moss_executor,
        "llm.": llm_executor,
    },
    default=moss_executor,  # Fallback for unprefixed tools
)

# Define a loop using prefixed tool names
loop = AgentLoop(
    name="hybrid_analysis",
    steps=[
        LoopStep(name="skeleton", tool="moss.skeleton.format"),
        LoopStep(name="analyze", tool="llm.analyze", input_from="skeleton"),
    ],
    entry="skeleton",
    exit_conditions=["analyze.complete"],
)

# Run
runner = AgentLoopRunner(composite)
result = await runner.run(loop, initial_input)
```

## With External MCP Server

```python
from moss.agent_loop import MCPServerConfig, MCPToolExecutor

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
        "moss.": moss_executor,
        "mcp.": mcp_executor,
        "llm.": llm_executor,
    }
)

# Loop can now use MCP tools
loop = AgentLoop(
    name="with_mcp",
    steps=[
        LoopStep(name="read", tool="mcp.read_file"),
        LoopStep(name="analyze", tool="moss.skeleton.format", input_from="read"),
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
| `moss.skeleton.format` | MossToolExecutor | `skeleton.format` | <!-- doc-check: ignore -->
| `mcp.read_file` | MCPToolExecutor | `read_file` |
| `llm.analyze` | LLMToolExecutor | `analyze` |

## Available MossAPI Tools

Tools available via `MossToolExecutor` (use with `moss.` prefix):

- `skeleton.format` - Get structural overview of a file
- `skeleton.extract` - Extract skeleton data structure
- `validation.validate` - Run syntax and linting checks
- `patch.apply` - Apply a patch to a file
- `patch.apply_with_fallback` - Apply patch with text fallback
- `anchor.find` / `anchor.resolve` - Find code anchors
- `dependencies.format` - Get dependency information
- `complexity.analyze` - Analyze code complexity
- `dwim.analyze_intent` - Find tools for a task

## Serialization

Hybrid loops can be serialized to YAML/JSON for version control:

```python
from moss.agent_loop import dump_loop_yaml, load_loop_yaml

# Save loop definition
yaml_str = dump_loop_yaml(loop)
dump_loop_yaml(loop, "loops/hybrid_analysis.yaml")

# Load loop definition
loop = load_loop_yaml("loops/hybrid_analysis.yaml")
```

Example YAML:

```yaml
name: hybrid_analysis
steps:
- name: skeleton
  tool: moss.skeleton.format
  step_type: tool
- name: analyze
  tool: llm.analyze
  step_type: llm
  input_from: skeleton
entry: skeleton
exit_conditions:
- analyze.complete
max_steps: 10
```

## Best Practices

1. **Use meaningful prefixes**: `moss.`, `mcp.`, `llm.` clearly indicate the tool source
2. **Set a default executor**: Handle unprefixed tools gracefully
3. **Connect MCP before running**: MCP executors require async connection
4. **Disconnect on cleanup**: Always disconnect MCP executors when done
5. **Use mock mode for testing**: Set `LLMConfig(mock=True)` for development

## Example: Full Hybrid Loop

See `examples/hybrid_loop.py` for a complete working example.
