# Ampcode Research

Agent coding tool by Sourcegraph. Research conducted 2025-12-23.

## Core Architecture

**Minimal agent loop** requires only three components:
1. LLM with tool access
2. Loop structure (prompt → inference → tool exec → repeat)
3. Sufficient tokens

Conversation history travels with each request. Server is stateless; client maintains complete context.

## Multi-Model Strategy

- **Worker** (Claude Sonnet 4): Fast, capable, handles bulk of tool use and code generation
- **Oracle** (GPT-5 / o3 / Gemini 2.5 Pro): High-level reasoning, architectural planning, debugging
- Oracle invoked explicitly ("ask the oracle"), not automatically

This is their answer to "which model is best" - use both, specialized by role.

## Subagent Architecture

Key insight: **Subagents multiply context windows.**

Instead of spending main agent tokens on a fix, spawn a subagent with fresh 200k context. Only a tiny fraction of main agent tokens used for the handoff.

Properties:
- Each subagent has isolated context window
- Full tool access (file edit, terminal, search)
- Can run in parallel
- Main agent receives only final summary (no lateral communication)

Model behavior: Claude Sonnet 4 aggressively uses subagents when it spots "clearly defined tasks". Earlier versions (3.5 Sonnet) rarely invoked them.

Specialized subagents:
- **Librarian**: Searches remote codebases (all public GitHub + private repos)
- **Generic subagents**: Mini-Amps with full capabilities

## Context Management

"Curated context beats comprehensive context. Every time."

Key decisions:
- **No automatic compaction** - risks quality degradation, users notice "agent got dumber"
- Manual control preferred over automatic threshold-triggered truncation
- Fixed 200k token window, aggressively leveraged
- Threads remain immutable; continuation uses explicit "handoff" with new context

## AGENTS.md Format

Open format for guiding coding agents. 60k+ open-source projects use it.

Structure:
- Standard Markdown, flexible sections
- Project overview, setup, build/test commands, code style, testing, security, PR guidelines
- Closest file in directory tree takes priority
- Supports YAML frontmatter with glob patterns for conditional inclusion

Precedence: explicit user prompts > AGENTS.md > defaults

## Tool Architecture

Layers:
1. **Built-in tools**: File editing, bash, git
2. **Toolboxes**: Scripts responding to `TOOLBOX_ACTION` env var (`describe`/`execute`)
3. **MCP servers**: Local/remote, OAuth-capable
4. **Skills**: Specialized instruction packages (SKILL.md + resources)

Permission system with sequential rule matching:
- Actions: `allow`, `reject`, `ask`, `delegate`
- Supports regex on tool names and arguments
- Can delegate to external programs

Edit tool uses **string replacement** (not diff) - Claude naturally prefers this.

## Operational Modes

- **Smart**: Opus 4.5, unconstrained tokens, 200k context
- **Rush**: Faster/cheaper for well-defined tasks
- **Free**: OSS models, ad-supported, some limits

Execute mode (`-x`): Non-interactive, full tool approval for autonomy.

## Design Philosophy (from FIF)

- **Transparency over abstraction**: Show agent work, don't hide complexity
- **Iterative loops over step approval**: Review → diagnostics → compiler → tests
- **No VCS manipulation**: Agent can't directly git, only shell commands with user direction
- **No .ampignore**: File hiding encourages workarounds; use permissions instead
- **No prompt injection defense** for Bash (noted as current limitation)

## Learnings from Sourcegraph

**Inversion of control**: Provide tools + high-level goals, let agent orchestrate. Don't micromanage via detailed prompts.

**Emergent capabilities**: Self-correction, test-driven iteration, parallel decomposition emerged from autonomy + feedback loops, not explicit design.

**Economics**: $5-15 per PR in observed usage. Optimizing for cost suppresses effectiveness.

**Human collaboration**: Agents implement rough versions, humans refine guardrails. Expertise is "moving guardrails" not prompting.

**Background agents**: 10-15+ minute tasks. Use CI pipelines as feedback (tests, linters) rather than replicating local env.

## Relevance to Normalize

Potential applications:
1. **Subagent pattern** for context multiplication - spawn fresh contexts for isolated tasks
2. **Oracle pattern** - dual-model with explicit escalation, not automatic
3. **String replacement** over diff for edits (already doing this)
4. **AGENTS.md equivalent** - CLAUDE.md serves similar purpose
5. **Curated context** - structural tools (view, analyze) align with this philosophy
6. **No compaction** - explicit handoffs over automatic truncation

Questions to explore:
- Should normalize TUI support spawning subagents?
- Oracle pattern worth implementing? (expensive model for stuck situations)
- Background agent support for long-running analysis?

## Sources

- https://ampcode.com/how-to-build-an-agent
- https://ampcode.com/manual
- https://ampcode.com/agents-for-the-agent
- https://ampcode.com/fif
- https://agents.md/
- https://www.nibzard.com/ampcode/
