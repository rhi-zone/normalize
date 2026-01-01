# Agent Design Notes

Brainstorming for moss agent implementation. Raw ideas, not vetted.

## Current State

`auto(config)` in `lua_runtime.rs`:
- Basic turn loop with `max_turns`
- Text-based command parsing (lines starting with `> `)
- Appends command output to conversation (grows forever)
- Calls moss CLI as subprocess

System prompt (minimal):
```
Tools:
view <path|symbol|path/symbol>
edit <path|symbol|path/symbol> <task>
analyze [--health|--complexity] [path]
grep <pattern> [path]
lint [--fix] [path]
shell <command>

Run: "> cmd". End: DONE
```

## Python Implementation (to port)

### TaskTree / TaskList

Old orchestration had hierarchical task structures:

```python
# TaskTree: hierarchical decomposition
task = TaskTree(
    goal="implement feature X",
    subtasks=[
        TaskTree(goal="understand current code", ...),
        TaskTree(goal="write implementation", ...),
        TaskTree(goal="add tests", ...),
    ]
)

# TaskList: flat sequence with dependencies
tasks = TaskList([
    Task(id="1", goal="read spec", deps=[]),
    Task(id="2", goal="implement", deps=["1"]),
    Task(id="3", goal="test", deps=["2"]),
])
```

Questions:
- Did hierarchical actually help? Or just added complexity?
- Was flat list sufficient in practice?
- How were dependencies tracked/resolved?

### Driver Protocol

Agent decision-making abstraction:

```python
class Driver:
    def decide(self, context: Context) -> Action:
        """Given current state, return next action."""
        pass

    def observe(self, result: Result) -> None:
        """Update internal state after action."""
        pass
```

Multiple driver implementations:
- `SimpleDriver`: Just call LLM each turn
- `PlanningDriver`: Plan first, then execute
- `ReflectiveDriver`: Periodically assess progress

### Checkpointing / Session Management

```python
session = Session.create(goal="...")
session.checkpoint("before risky edit")
# ... do work ...
if failed:
    session.rollback("before risky edit")
```

Now we have `shadow.*` for this.

## Open Questions

### Tool Invocation Format

**Assumption to question:** "Structured tool calls (JSON) are better than text parsing"

Counter-argument: LLMs are trained on text. Shell commands and prose are native. JSON schemas are learned later. Which actually works better?

Options:
1. **Text/shell style**: `> view src/main.rs --types-only`
2. **XML tags**: `<tool name="view"><arg>src/main.rs</arg></tool>`
3. **JSON**: `{"tool": "view", "args": {"target": "src/main.rs"}}`
4. **Function calling API**: Provider-specific (Anthropic tool_use, OpenAI function_call)
5. **Prose**: "Please show me the structure of src/main.rs"

Factors:
- Reliability of parsing
- Token efficiency
- Model familiarity (training data distribution)
- Provider compatibility

**Experiment needed**: Same task with different formats, measure success rate.

### Context Management

Current: append everything, let context window fill up.

Problems:
- Early context gets "lost in the middle"
- Irrelevant output stays in context
- No prioritization

Ideas:
- **Sliding window**: Keep last N turns, summarize earlier
- **Priority queue**: Score context chunks by relevance to current task
- **Structured context**: Always include { task, current_file, recent_errors }
- **Memory offload**: Use `store()`/`recall()` to persist across turns

### Planning vs Reactive

Two modes:
1. **Reactive**: Each turn, decide what to do next based on current state
2. **Planning**: Produce a plan upfront, then execute steps

Trade-offs:
- Planning: Better for known workflows, worse when plan is wrong
- Reactive: More flexible, but can loop/wander
- Hybrid: Plan loosely, revise as you go

### Loop Detection

Agents get stuck. Signs:
- Same action repeated 3+ times
- Same error message recurring
- No progress toward goal

Responses:
- Reflect ("I'm stuck because...")
- Backtrack (rollback to checkpoint)
- Escalate (ask user for help)
- Give up (exit with explanation)

### Cost Awareness

Current: no visibility into token usage.

Options:
- Track tokens per turn, show running total
- Budget enforcement (stop at N tokens)
- Cost-aware action selection (cheap probes before expensive operations)

## Integration with Existing Primitives

### shadow.* for Rollback

```lua
shadow.open()
local before = shadow.snapshot({"src/"})

-- agent works...
result = agent.execute(task)

if not result.success then
    shadow.restore(before)
    print("Rolled back due to failure")
end
```

### store/recall for Memory

```lua
-- After learning something
store("The auth module uses JWT tokens in cookies", {
    context = "architecture",
    weight = 0.8
})

-- Before starting a task
local hints = recall(task.description, 5)
for _, h in ipairs(hints) do
    context:add(h.content)
end
```

### Investigation Flow

From dogfooding notes:
```
view . → view <file> --types-only → analyze --complexity → view <symbol>
```

Agent should learn this pattern, not rediscover it each time.

## Adaptation (from agent-adaptation.md)

Moss tools are **T1** (agent-agnostic). They improve independently:
- Index refresh via file watching
- Grammar updates
- Output format improvements

Not doing **A1/A2** (agent adaptation) - that requires fine-tuning LLMs, outside scope.

**T2** (agent-supervised tool adaptation) is the interesting edge:
- Observe agent friction (repeated queries, workarounds, corrections)
- Adjust tool defaults based on usage patterns
- No LLM fine-tuning, just tool improvement

## Related Files

- `crates/moss/src/workflow/lua_runtime.rs` - Current `auto()` implementation
- `crates/moss/src/workflow/llm.rs` - LLM client, system prompt
- `docs/research/agent-adaptation.md` - Adaptation framework analysis
- `docs/lua-api.md` - Available Lua bindings

## Next Steps

1. Decide on tool invocation format (experiment?)
2. Design context management strategy
3. Port TaskTree/TaskList concepts if useful
4. Add cost tracking
5. Implement loop detection
