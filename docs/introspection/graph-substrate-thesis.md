# Graph Substrate for Stateful LLM Workflows

A thesis on why current LLM-coding workflows lose state, misaim work, and pay recurring correction costs — and what a small primitive in `normalize` could do about it.

This document was authored in a single Claude Code session on 2026-05-20. It captures a line of reasoning that emerged from an investigation into "Claude Code is performing worse" — which the investigation itself partially refuted, then opened into deeper questions about context, state, and decomposition. The reasoning here is honest but not battle-tested. Treat it as a starting position for the actual design work in `docs/design/graph-substrate.md`, not a finished argument.

## Origin: the investigation that started this

The presenting complaint: Claude Code appears to be performing worse, especially on crescent's typechecker (which has been rewritten 3+ times). An ecosystem-wide investigation ran 10 hypotheses through populating + adversarial-red-team agents, then Opus adjudication. The presenting framing turned out to be largely wrong:

- **H-MODEL-REGRESSION**: dead. Zero version-named complaints; all model switches are toward Opus.
- **H-DESIGN-CEILING**: dead. The crescent typechecker has been *four architecturally distinct generations* (HM unification → constraint-based → two-phase → set-theoretic / MLstruct-style), not three repeats of the same mistake. That is shipped architectural evolution under high friction, not capability collapse.
- **H-PROMPT-SHAPE**: dead. Rescribe (largest CLAUDE.md in the ecosystem) has the highest tool-success rate. The frustration → CLAUDE.md bloat arrow runs in the opposite direction from what the hypothesis claimed.
- **H-MOMENTUM-LOSS**: weak. 20-day project gaps are ecosystem-normal.
- **H-GOVERNANCE-BREACH**: trace-level (~0.1% of commits).

What survived, and what the rest of this document is about:

- **H-IMPLICIT-CONSTRAINTS** (strong): the user's aesthetic exists largely as undocumented preferences mined post-violation. Crescent committed 98 CLAUDE.md changes in 60 days, ~1.6/day, with no saturation.
- **H-CORRECTION-TAX** (narrowed): documented rules sometimes work and sometimes don't. The May 13 "don't fake confidence" rule actually suppressed that violation class — zero recurrences after it landed. Other documented rules (no-bandaid, no-specialcase) keep being violated. The pattern: **rules that *name a failure mode* work; rules that state a principle whose application requires running the user's generator do not.**
- **H-CONTEXT-DRIFT + H-DECOMPOSITION-FAILURE** (both narrowed, same mechanism at different scales): compaction silently strips prior agreements; subagent orchestrators re-dispatch on stale assumptions about what prior agents accomplished. "Long sessions degrade" is dead (641-message sessions show zero pushback) — what survives is orchestrator-state decay across context transitions.
- **H-CACHE-MASKS-DEGRADATION** (confirmed as epistemic constraint): cache reuse is 99.99%, so aggregate cost/volume metrics are functionally blind to model quality.

The accurate reframe: **the model is roughly fine. The transmission channel between the user's aesthetic and each fresh session is the bottleneck.** What looks like "performing worse" is the user paying recurring costs to push the same generator through CLAUDE.md, one violated rule at a time.

That observation opens the deeper question this document addresses: what is the right substrate for that transmission channel?

## The thesis

**Conversation-as-substrate is broken. Reasoning happens in an append-only forward token stream that cannot unpoison itself once contaminated — premature conclusions become load-bearing on everything downstream, intent drifts and cannot be steered back in-context. The fix is to move state out of conversation context entirely, into a persistent graph substrate that (a) survives context discard so kill-and-restart becomes cheap, and (b) serves as the full-fidelity intent channel for stateless subagent calls so decomposed work doesn't lose information through prompt-shaped compression.**

This is largely consistent with [nanites' thesis](../../nanites/CLAUDE.md) (conversation is context poisoning; the agent is the wrong unit; recursive decomposition is the architecture; the orchestrator is a program, not an agent). Nanites articulates the orchestration story. This document articulates the *state* story that orchestration needs to actually work. They're complementary.

## Sub-claims and the reasoning

### 1. Per-token compute is fixed; therefore reasoning depth is bounded by token count

Each token requires roughly fixed compute (one forward pass). Total reasoning in any response is therefore proportional to total tokens produced, multiplied by a constant. There is no "concentrate harder" knob — each token gets the same compute whether it's filler or critical insight.

Direct consequences:

- **Terse output to hard problems is mechanically under-reasoned.** "Concise wisdom" is a category error for LLMs. If a problem needs K units of reasoning and the response is K−1 tokens, the answer is wrong by construction.
- **Any conclusion at token N has had at most N×C compute behind it.** Early conclusions are necessarily less considered than late ones.
- **The model has no internal sense of how much compute a given problem needs**, so it cannot refuse to conclude early. It concludes when the next-token distribution says "conclude now."

### 2. Premature conclusions are context poisoning, not just suboptimal

Once a conclusion enters context, every subsequent token is conditioned on it. There is no in-context operation that removes a prior commitment. "Revision" produces text-after-the-poison, not text-without-it. The model can't tell which prior context to trust and which was provisional — it treats everything as load-bearing premise.

Therefore: emitting a final answer before sufficient preparatory reasoning has occurred is *objectively never the right move*. It contaminates every downstream token. The contamination compounds: each premature conclusion shapes subsequent reasoning, which shapes the next conclusion, and so on.

This is why mid-stream corrections don't work. Adding "actually, X was wrong" doesn't delete X from context — the model now has both, and prior derivations from X still bias subsequent generation. Corrections accumulate as additional constraints, not as resets.

### 3. Context poisoning isn't about distraction, it's about intent divergence

The deeper formulation: the model uses accumulated context to infer *what it should be doing*. Bad context misrepresents intent, and the model then pursues the wrong goal with full competence. High-competence execution of a slightly-wrong goal looks identical to low-competence execution of the right goal from the outside.

Every statement in main context — exploratory hedging, tentative framings, casual "let me try X" — shapes the model's inferred intent. The hedge language doesn't help; the model conditions on the surface form. Corrections to outputs don't reset intent; they add new constraints to the existing (wrong) inference, and the model tries to satisfy the stack, drifting sideways.

This means **steering doesn't work in a poisoned context.** Once intent has drifted, it doesn't return via in-context correction. The only repair is *killing the context* and starting fresh with intent stated cleanly.

### 4. Problem-solving is recursive decomposition; per-problem compute is unbounded

Problem-solving is not forward generation, it is exploration with backtracking: try, fail, discard, try differently, iterate. Discarded attempts are most of the work. The final answer is what's left after pruning.

LLM token generation cannot do this — every token sticks. But the recursion bypasses the per-turn compute limit entirely. Each sub-problem can be solved in its own inference with its own fresh N tokens of compute. K levels of decomposition with B branches per level gives ~B^K × N tokens of total reasoning, with independent contexts. The per-turn ceiling is a workflow property, not a model property.

Furthermore, decomposition is itself decomposable. If a problem looks atomic, you spawn the sub-problem "find a decomposition for this," which is itself decomposable. There is no class of problem that resists this. **Reasoning depth has no fundamental upper bound** given recursion and willingness to spend compute.

There is overhead, though: spawning has cost. For simple problems, decomposition is more expensive than direct solving. So the actual skill is **calibrating when to decompose**, not "always decompose." Asymmetry: under-decomposing a hard problem wastes the whole attempt; over-decomposing a simple one wastes some compute but still yields an answer. When uncertain, err toward decomposition.

### 5. Subagents contain context poisoning but inherit it internally — and prompt-as-sole-bridge is the deepest weakness

Subagents solve part of the problem. They isolate context — a subagent's failed reasoning stays in its context, only the result returns to parent. Main session sees a summary, not the poisoned intermediate work. Recursive decomposition via subagents IS how compute multiplies and how backtracking happens at coarse granularity.

But subagents have all the same internal limitations: premature commitments, intent divergence, monotonic context. They don't escape context poisoning, they contain it. Useful but partial.

The *deepest* weakness is **prompt-as-sole-bridge**. The subagent's behavior is entirely determined by its initial prompt. Anything in main session's context that should inform the subagent has to be re-articulated in the prompt. Rich accumulated context → compressed to a dispatch prose → subagent operates on a lossy snapshot. Subagents misaim not because they're dumb but because what they received wasn't intent, it was a compression of intent.

Everything else about subagent weakness (fire-and-forget, lossy summary return, can't course-correct mid-flight) is downstream of this single failure: prompts as the only intent channel.

### 6. The persistent substrate is the missing piece

If state lives *outside* any conversation — in a persistent, addressable graph — both context poisoning and the subagent-bridge weakness can be addressed:

- **Context discard becomes cheap.** Killing a poisoned context loses nothing because the state was already written to the substrate. Fresh contexts read the relevant slice and resume. Discard stops being the expensive last resort and becomes a routine tool.
- **Subagent prompts collapse.** Instead of compressing main session's accumulated context into a prose dispatch, the subagent gets `your node id; here's your task; the rest is in the substrate.` The subagent reads its parent, siblings, prior decisions, evidence, and root intent *directly*, at full fidelity. The fragile prose bridge is replaced by direct read access.
- **Knowledge compounds across sessions.** Insights stop being lost on session end. The substrate is the long-term memory.

The substrate is not specifically a work queue, problem tree, decision log, investigation registry, or knowledge base. It is the *underlying primitive* of which all of these are conventions on top: structured nodes with metadata and addressable cross-references, queryable, persistent.

### 7. The right primitive is a graph, not a tree

A tree without a designated root is just a graph. Trees with imposed root + acyclicity break immediately on common cases: shared evidence across hypotheses, sub-problems serving two parents, constraints derived from multiple sources. The natural primitive is graph (nodes + edges + metadata, queryable) with tree/DAG-shaped subgraphs as patterns rather than as the substrate's own assumption.

Wiki shape is closer to right than tree: pages with free-form content, addressable IDs, cross-references forming a graph from usage rather than from imposed hierarchy. Wikipedia, Notion, Obsidian validate the pattern at scale for collaborative human knowledge. Should extend to human + LLM collaboration with appropriate metadata and queryability.

### 8. Updates must be a side effect of doing the work, not an optional discipline

A substrate that depends on someone remembering to update it is the same as TODO.md — fine in theory, skipped under pressure, stale within days. For the substrate to actually serve as the durable layer, *updates must be unavoidable consequences of dispatch*, not extra steps.

Mechanisms (specific to the Claude Code harness, since normalize doesn't ship with one — see scope note below):

- Subagent dispatch is bound to a node. Subagent return is written back to that node by the dispatch wrapper, not by the subagent remembering.
- Session-start automatically loads the relevant subgraph into context.
- Session-end (or context-discard) flushes any unwritten state into the graph.
- Plan-mode / handoff transitions auto-persist before discarding.

Without these, the substrate is just another markdown directory. With them, the substrate is the source of truth and conversation is a transient viewport into it.

## Scope note: what's in normalize vs what's in the harness

Normalize doesn't ship with Claude Code integration. The harness-level pieces (hooks for session-start load, dispatch-bound write-back, end-of-session flush) live in shell wrappers around Claude Code, not in normalize itself. Normalize's responsibility is the substrate: graph nodes + edges + metadata, queryable, link-resolvable. Conventions on top (workqueue, decomposition tree, investigation registry, design space exploration, decision log, etc.) live in the calling layer.

This separation matters because it keeps normalize small and general, and lets the integration evolve without changing the substrate.

## Connections to existing work in the ecosystem

- **Nanites' CLAUDE.md** articulates the correct orchestration story (conversation-as-poisoning, function-call-as-primitive, recursive decomposition, LLM-as-oracle, orchestrator-as-program). It does not articulate the persistent substrate that orchestration depends on. This document fills that gap.
- **`normalize context`** is conceptually adjacent: filesystem-backed, hierarchical-resolvable, markdown-with-frontmatter, metadata-filterable. It is a context-injection substrate, not (yet) a graph-shaped one — it lacks cross-references between blocks and doesn't model addressable nodes. The graph substrate could share machinery with `normalize context` (or be its generalization) but is a distinct concept.
- **TODO.md and CLAUDE.md** are the current ad-hoc state-tracking conventions in every ecosystem repo. They handle flat backlog and invariants respectively. They do not handle live decomposition state, cross-session knowledge, or queryable evidence accumulation. Their limitations motivate this proposal.
- **`docs/introspection/`** holds retrospective summaries (daily logs, weekly syntheses). It does not host *prospective* state (active work queues, open hypotheses, pending decompositions).

## Open questions / nuance

Items where the reasoning in-session was either incomplete, contested, or deliberately deferred:

- **Schema for nodes.** Frontmatter shape (id, type, status, parent links, intent, derived-from) is not designed yet. Wrong schema baked in early would be expensive to change.
- **Query language.** "Find pending nodes under root X with status=open and metadata.priority>=2" is the kind of query needed. Whether to extend `normalize context`'s `--match` system or to add something new is open.
- **Storage format.** Markdown files with frontmatter (`normalize context`-style) vs SQLite (structured, query-efficient) vs both. The user experience of editing nodes (vs reading and writing them programmatically) shapes this.
- **Cross-reference resolution.** Wikilink-style `[[node-id]]`? Markdown links? Custom syntax? The choice affects ergonomics for both human editors and parsers.
- **Initialization and migration.** How existing TODO.md, CLAUDE.md, introspection logs migrate into the substrate. Probably gradual — substrate stands up alongside, conventions move over one at a time.
- **Calibration of when to decompose.** Even with the substrate, the orchestrator needs to know when a problem warrants decomposition. The simple-problem-overhead issue from sub-claim 4 doesn't go away.
- **Generator-vs-rules in encoding constraints.** The investigation surfaced that some constraints encode cleanly as named failure modes ("don't fake confidence") and some don't (because they require running the user's generator). The substrate doesn't directly solve this, but it might make it more tractable by providing a place for accept/reject example pairs that would otherwise be cringe-shaped few-shot prompting.
- **Research-shaped vs engineering-shaped work.** The substrate itself is engineering-shaped (well-defined, existing prior art). But it's intended to *serve* work that often is research-shaped (typechecker design). It will not magically make research-shaped work easy; it will make the workflow around it less wasteful.
- **Whether the harness integration is feasible as shell-wrapper-only**, or whether it eventually needs proper hooks/plugin support in Claude Code. Shell wrappers can do a lot but have limits (e.g., they can't easily intercept tool-use mid-response).

## What was rejected along the way

For posterity, since the reasoning that produced these rejections is itself part of the thesis:

- **Few-shot examples / generator-corpus** as a fix for hard-to-encode constraints. Cringe; pattern-matches surface features; doesn't capture the generator. Replaced by "name failure modes precisely" as the rule-format discipline.
- **Specialized `normalize workqueue` / `normalize problemtree` subcommands.** Encodes specific assumptions, fragments the surface, misses the common substrate. Replaced by general graph primitive with conventions on top.
- **Tree as the substrate's primitive shape.** Forbids multi-parent, imposes hierarchy that breaks common cases. Replaced by graph.
- **Model switching as the fix.** Opus-as-orchestrator was suggested by the investigation but the user reported it didn't help crescent — confirming that the bottleneck isn't model class at the orchestrator role; it's the rule-format + implicit-constraint stack.
- **More CLAUDE.md rules.** Rules-vs-generators issue; reactive-bandaid loop (rules added in response to violations themselves violate the no-bandaid principle); 98 commits/60 days with no saturation.
- **In-context self-correction.** Structurally impossible because context is append-only and corrections add to the poison rather than removing it. Replaced by context discard as the primary repair tool, which requires the substrate to be viable.
- **Subagent-as-complete-solution.** Subagents contain context poisoning but don't escape it; their biggest weakness (prompt-as-sole-bridge) is exactly what the substrate solves.

## What this thesis does not claim

- That implementing this substrate will make Claude Code (or any LLM tool) good at type-system design. The investigation showed that crescent's typechecker work is partly research-shaped (Lua doesn't fit any standard calculus cleanly), and no workflow primitive bridges research-shaped gaps. The substrate addresses the *transmission* problem, not the underlying capability problem.
- That the substrate substitutes for the user's taste. The user remains the authoritative source of intent and the arbiter of correctness. The substrate makes that authority easier to transmit, not unnecessary.
- That this is the only architectural improvement available. The harness still has real limitations (no surgical message deletion; no first-class branching of contexts; no native task-tree primitive). Those would also help. This is the highest-leverage missing piece, not the only one.

## Next steps

If this thesis holds up under scrutiny:

1. Draft a design document for the actual primitive (`docs/design/graph-substrate.md`). Schema, query language, storage format, API surface, integration points.
2. Prototype the smallest version that's useful — probably a `normalize graph` subcommand backed by SQLite or markdown-with-frontmatter, with node/edge/query primitives. Migrate one convention onto it (likely investigation registries first, since that's the convention with the clearest schema already).
3. Build the shell-wrapper integration with Claude Code's hooks (session-start auto-load, dispatch-bound write-back).
4. Migrate other conventions (work queues, decision logs, decomposition state) one at a time as their schemas stabilize.
5. Re-run the kind of investigation that started this thesis after six months of substrate use, to see whether the alive hypotheses (correction tax, implicit constraints, orchestrator-state decay) measurably shrink.
