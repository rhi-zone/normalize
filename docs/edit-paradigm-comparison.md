# Edit Paradigm Comparison: Claude Code vs Gemini CLI

Investigation into why edit quality differs between the two CLI agents.

## Tool Paradigms

### Claude Code Edit Tool
- **Approach**: Strict exact string matching
- **Parameters**: `file_path`, `old_string`, `new_string`, `replace_all`
- **Uniqueness**: `old_string` MUST be unique or edit fails
- **On failure**: Immediately fails, expects model to retry with more context
- **MultiEdit**: Separate tool for batching edits to same file

Source: [Claude Code system prompts](https://github.com/Piebald-AI/claude-code-system-prompts)

### Gemini CLI Edit Tool
- **Approach**: Multi-strategy with self-correction
- **Parameters**: `file_path`, `old_string`, `new_string`, `expected_replacements`
- **Uniqueness**: Soft - uses occurrence count
- **On failure**: `ensureCorrectEdit()` invokes LLM to fix parameters and retry
- **SmartEdit**: 3-stage fallback: exact → flexible (ignores whitespace) → regex (token-based)

Source: [Gemini CLI tools](https://github.com/google-gemini/gemini-cli/tree/main/packages/core/src/tools)

## Key Differences

| Aspect | Claude Code | Gemini CLI |
|--------|-------------|------------|
| Philosophy | Fail fast, be precise | Self-correct, be flexible |
| Whitespace | Must match exactly | Flexible matching |
| Self-correction | None (model retries) | LLM-powered fix loop |
| Context requirement | User adds context | Tool adds context |

## Hypotheses for Quality Differences

### Why Claude Code may produce better edits:
1. **Strict matching forces precision** - Model must think carefully about exact content
2. **No fuzzy matching** - What you specify is what changes, no surprises
3. **Explicit failure** - Clear signal when something is wrong

### Why Gemini CLI may produce worse edits:
1. **Self-correction may accept wrong edits** - "Close enough" isn't correct
2. **Flexible whitespace matching** - Can silently change formatting
3. **Regex fallback** - May match unintended locations
4. **Hidden complexity** - Model doesn't learn from failures if tool auto-fixes

## Implications for Normalize

Normalize should follow the Claude Code pattern:
- Strict matching with explicit failures
- Force the model to be precise
- No self-correction that hides problems
- Anchor-based editing is even stricter (structural, not textual)

## Further Investigation

- [ ] Parse session logs to measure edit success rates
- [ ] Compare before/after diffs for unintended changes
- [ ] Test same edit task on both tools, compare results
- [ ] Check if Gemini's SmartEdit regex matching causes false positives
