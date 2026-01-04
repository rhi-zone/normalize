# Agent Prompt Experiments

Tracking prompt iterations and their effectiveness at preventing pre-answering.

## Problem Statement

LLMs pre-answer: they output commands and `$(done)` in the same turn, answering before seeing command results. This happens because LLM training rewards task completion - if the task *looks* complete in 1 turn, the LLM does 1 turn.

**Root cause**: Task framing makes single-turn look like correct completion.
**Fix direction**: Reframe task so multi-turn IS correct completion.

## Baseline: Original Prompt (pre-8e33340)

Unknown state. No logs available.

## Experiment 1: Simple Prompt (commit 8e33340)

```
Coding session. Output commands in [cmd][/cmd] tags. Conclude quickly using done.
[commands]
[cmd]done answer here[/cmd]
...
```

**Model**: Gemini Flash
**Result**: "Flash typically needs 4-5 turns but now concludes reliably"
**Context model**: Conversational (append-only chat history)
**Analysis**: Simple prompt, no memory management. Worked but used problematic conversational model.

## Experiment 2: Memory Management + $(wait) (current, pre-investigator)

```
Coding session. Output commands in $(cmd) syntax. Multiple per turn OK.

Command outputs disappear after each turn. To manage context:
- $(keep) or $(keep 1 3) saves outputs to working memory
- $(note key fact here) records insights for this session
...
- $(wait) waits for command results before answering

IMPORTANT: If you issue commands that produce the answer, use $(wait) to see results first.
DO NOT call $(done) in same turn as commands that contain the answer.
```

**Model**: Various (Claude, Gemini)
**Result**: Pre-answering still occurs. $(wait) is a band-aid (post-processing).
**Context model**: Ephemeral (1-turn visibility window)
**Analysis**:
- Adding $(wait) instruction doesn't prevent pre-answering
- Complex memory management may distract from core task
- Prompt says "don't do X" but doesn't reframe what correct completion looks like

## Experiment 3: Investigator Role

### 3a: Initial (verbose, no example)

First attempt used verbose prompt with many memory commands. Claude ignored `$(cmd)` syntax entirely - used XML function calls and hallucinated the answer.

**Session**: renuh3aq
**Model**: Claude (anthropic)
**Result**: FAILURE - Used XML syntax, hallucinated fake file names

### 3b: Simplified with concrete example (current)

```
You are a code investigator. Gather evidence, then conclude.

Output commands using $(command) syntax. Example turn:
$(view .)
$(text-search "main")
$(note found entry point in src/main.rs)

WORKFLOW:
1. GATHER - Run commands to explore
2. RECORD - $(note) findings you discover
3. CONCLUDE - $(done answer) citing evidence

Commands:
$(view path) $(view path/symbol) $(view --types-only path)
$(text-search "pattern")
$(run shell command)
$(note finding)
$(done answer citing evidence)

Outputs disappear each turn unless you $(note) them.
Conclusion must cite evidence. No evidence = keep investigating.
```

**Hypothesis**: Concrete example + role framing + evidence requirement = multi-turn behavior.

### Design Rationale

1. **Concrete example**: Shows exact syntax - prevents XML function call default
2. **Role framing**: "investigator" makes gathering evidence THE job
3. **Workflow steps**: Multi-turn baked into structure (GATHER → RECORD → CONCLUDE)
4. **Evidence requirement**: "Conclusion must cite evidence" - positive framing
5. **Simplified commands**: Only essential commands listed
6. **No negative instructions**: No "don't do X", only positive guidance

---

## Session Log

Format: `session_id | model | task | turns | correct | notes`

| Session | Model | Task | Turns | Correct | Notes |
|---------|-------|------|-------|---------|-------|
| renuh3aq | claude | count lua scripts | 1 | NO | 3a prompt: Used XML syntax, hallucinated answer |
| 84gmtqny | claude | count lua scripts | 2 | YES | 3b prompt: Correct syntax, answer, cited evidence |
| s9evceus | claude | find Anthropic default model | 8 | YES | Took many turns, some looping on $(view .), but correct answer with line citation |
| g4n93rvr | claude | count Provider enum variants | 5 | YES | Correct: 13 variants, all named, cited line numbers |

### Summary (Experiment 3b)

**Results**: 3/3 correct with new prompt (vs 0/1 with 3a prompt)
**Turns**: 2-8 (multi-turn as intended, no pre-answering)
**Key insight**: Concrete example in prompt prevents LLM from defaulting to XML function calls

**Remaining issues**:
- Session s9evceus showed some looping (viewed "." twice)
- Turn count varies widely (2-8)
- Gemini untested (API blocked in this environment)

