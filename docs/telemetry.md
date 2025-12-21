# Telemetry Design

## Purpose

Primary: Diagnostics - understand agent behavior, find hotspots, identify improvement opportunities.
Secondary: Cost tracking - tokens, API calls, time.

## Key Insight

We have the codebase tree. Telemetry should leverage this:
- Tokens spent per function/file/module
- Access patterns mapped to code structure
- Hotspots correlated with complexity metrics

## Architecture

**Unified log model**: All session formats are plugins.
- Claude Code JSONL
- Gemini CLI logs
- Moss internal sessions
- Cline/Roo/Aider formats

Moss sessions are "first-class" only in that we capture maximal metadata, but implementation is still a plugin like any other format.

**Plugin interface**: Each format parser produces a common `SessionData` structure.

## Audiences

- Developer: debugging their own sessions
- Team lead: analyzing patterns across sessions
- Agent: self-improvement feedback loop (memory system integration)

## Modes

- Post-hoc: `moss telemetry` CLI, HTML dashboards
- Real-time: live metrics during session (future: TUI integration)

## Data Model

Core metrics:
- Token usage (input, output, cache)
- Tool calls (name, success/fail, duration)
- File access patterns
- Error patterns and retries

Codebase-aware metrics:
- Tokens per symbol path (e.g., `src/foo.py/MyClass/method`)
- Read/write ratio per file
- Complexity vs access correlation

## CLI Design

```
moss telemetry                    # Aggregate stats (all moss sessions)
moss telemetry -s <id>            # Specific moss session
moss telemetry -l *.jsonl         # External logs (auto-detect format)
moss telemetry --html             # Dashboard output
moss telemetry --watch            # Real-time mode (future)
```

## Integration Points

- TelemetryAPI: programmatic access
- SessionAnalyzer: log parsing (becomes a plugin)
- CodebaseTree: map access patterns to structure
- Memory system: feed insights for cross-session learning
