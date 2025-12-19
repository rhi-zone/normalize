# Recursive Self-Improvement

Loops that analyze and improve other loops. This is meta-programming for agent workflows.

## Concept

Traditional loops execute a fixed sequence of steps. Recursive improvement loops can:
1. Analyze their own structure
2. Identify inefficiencies
3. Suggest or apply improvements
4. Learn from execution history

## Available Meta-Loops

### loop_critic_loop

Analyzes a loop definition and produces improvement suggestions.

```python
from moss.agent_loop import loop_critic_loop, dump_loop_yaml

# Get a loop to analyze
target_loop = docstring_loop()
loop_yaml = dump_loop_yaml(target_loop)

# Run the critic
critic = loop_critic_loop()
runner = AgentLoopRunner(executor)
result = await runner.run(critic, initial_input=loop_yaml)

# Result contains structured analysis and suggestions
print(result.context.get("suggest"))
```

Analysis checks for:
- Missing error handling (steps without `on_error`)
- Potential infinite loops (cycles without exit conditions)
- Inefficient step ordering
- Missing validation steps
- Unclear step purposes

### loop_optimizer_loop

Optimizes a loop for token efficiency.

```python
from moss.agent_loop import loop_optimizer_loop

optimizer = loop_optimizer_loop()
result = await runner.run(optimizer, initial_input=loop_yaml)

# Result contains optimized loop YAML
optimized_yaml = result.context.get("optimize")
optimized_loop = load_loop_yaml(optimized_yaml)
```

Optimization targets:
- Reduce LLM calls where tool calls suffice
- Combine mergeable steps
- Add caching hints
- Remove redundant validation

### self_improving_docstring_loop

A docstring loop with built-in self-critique and improvement.

```python
from moss.agent_loop import self_improving_docstring_loop

loop = self_improving_docstring_loop()
result = await runner.run(loop, initial_input=file_path)

# Loop self-corrects if quality is low
# critique.score < 7 triggers improvement step
```

Flow:
1. Extract skeleton
2. Generate docstrings
3. Self-critique (score 1-10)
4. If score < 7, improve based on critique
5. Exit when quality sufficient

## Building Custom Meta-Loops

### Pattern: Critique → Improve

```python
def my_meta_loop():
    return AgentLoop(
        name="meta",
        steps=[
            LoopStep("analyze", "llm.analyze", step_type=StepType.LLM),
            LoopStep(
                "improve",
                "llm.improve",
                input_from="analyze",
                step_type=StepType.LLM,
            ),
        ],
        exit_conditions=["improve.success"],
    )
```

### Pattern: Measure → Optimize

```python
def optimization_loop():
    return AgentLoop(
        name="optimize",
        steps=[
            LoopStep("measure", "tool.measure", step_type=StepType.TOOL),
            LoopStep(
                "identify",
                "llm.find_waste",
                input_from="measure",
                step_type=StepType.LLM,
            ),
            LoopStep(
                "optimize",
                "llm.rewrite",
                input_from="identify",
                step_type=StepType.LLM,
            ),
        ],
        exit_conditions=["optimize.success"],
    )
```

### Pattern: Generate → Validate → Retry

```python
def self_correcting_loop():
    return AgentLoop(
        name="self_correct",
        steps=[
            LoopStep("generate", "llm.generate", step_type=StepType.LLM),
            LoopStep(
                "validate",
                "validation.validate",
                input_from="generate",
                step_type=StepType.TOOL,
            ),
            LoopStep(
                "fix",
                "llm.fix_errors",
                input_from="validate",
                step_type=StepType.LLM,
                on_error=ErrorAction.SKIP,
            ),
        ],
        exit_conditions=["validate.success"],
        max_steps=5,  # Prevent infinite retry
    )
```

## Design Principles

### 1. Bounded Recursion

Always set `max_steps` to prevent infinite loops:

```python
AgentLoop(
    ...,
    max_steps=10,  # Safety bound
)
```

### 2. Clear Exit Conditions

Define when improvement is "good enough":

```python
exit_conditions=[
    "validate.success",      # Validation passed
    "critique.score >= 8",   # Quality threshold
    "iterations > 3",        # Iteration limit
]
```

### 3. Graceful Degradation

Use `on_error=ErrorAction.SKIP` for optional improvement steps:

```python
LoopStep(
    "improve",
    "llm.improve",
    on_error=ErrorAction.SKIP,  # Don't fail if improvement fails
)
```

### 4. Preserve Original

Keep the original input for comparison:

```python
steps=[
    LoopStep("original", "identity", step_type=StepType.TOOL),
    LoopStep("improved", "llm.improve", input_from="original"),
    LoopStep("compare", "llm.compare", input_from=["original", "improved"]),
]
```

## Use Cases

| Scenario | Meta-Loop |
|----------|-----------|
| Review loop definitions | `loop_critic_loop` |
| Reduce token costs | `loop_optimizer_loop` |
| Quality-aware generation | `self_improving_docstring_loop` |
| Prompt refinement | Custom critique loop |
| Workflow optimization | Custom measure-optimize loop |

## Metrics

Track improvement effectiveness:

```python
from moss.agent_loop import LoopMetrics

# Before optimization
metrics_before = runner.run(original_loop, input).metrics

# After optimization
metrics_after = runner.run(optimized_loop, input).metrics

# Compare
token_reduction = 1 - (metrics_after.total_tokens / metrics_before.total_tokens)
print(f"Token reduction: {token_reduction:.1%}")
```

## Limitations

1. **LLM-dependent**: Meta-analysis requires LLM calls
2. **Not guaranteed**: Improvements are suggestions, not proven optimal
3. **Context-dependent**: What's optimal depends on use case
4. **Cost trade-off**: Running meta-loops costs tokens

Use meta-loops for:
- One-time optimization of frequently-used loops
- Quality gates on generated content
- Iterative refinement workflows

Don't use for:
- Simple, well-defined tasks
- Latency-critical paths
- Low-value improvements
