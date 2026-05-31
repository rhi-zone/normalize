# Inferred Opinionation: Guess Configuration / Taste / Consensus for Free

Design notes for inferring canonical forms from the codebase itself — opinionation
without hand-written rules.

## Status

Backlog — not yet started.

---

## Core Idea

Today, canonical forms must be supplied externally — a reference file, an eglint
config, a manually curated rule set. "Format one file to format them all" is already
a step forward, but a codebase is not a singular example. It is a *corpus*. The
distribution of choices already encoded in the codebase IS the configuration — we
just haven't read it.

The goal: **infer style conventions automatically from the corpus via heuristics**,
with no hand-written rules. Opinionation guessed for free.

---

## Configuration by Consensus

For each decision-class (formatting, control-flow style, expression ordering,
delimiter choices, etc.), the codebase is a distribution of choices. The convention
is the consensus over all instances.

**Scope: style / formatting / control-flow only.** Style has no "correct," only
"consistent" — consensus = consistency. You cannot enshrine a "wrong" style.

**NOT for semantic / correctness canonicalization.** There the modal pattern can be
the modal *bug* — enshrining LLM slop. Semantic consensus is unsafe.

---

## Distribution Shape, Not Binary

The per-class vote is a distribution over *k* forms, not a binary. The number of
significant modes *m* is the blessed set (tied to the broader "collapse k→m"
framing):

| Shape | m | Action |
|-------|---|--------|
| Sharp unimodal | 1 | Collapse all instances to the one form |
| Multimodal | #peaks | Legitimate forms — keep all; collapse only the off-peak tail |
| Flat | ? | Free decision (no convention) or chaos — see below |

The flat case is resolved by the decision-tree step.

---

## Decision-Tree Formalization

The crux: learn a **per-decision-class decision tree**.

- **Features = structural context**: parent construct, scope type, nesting depth,
  sibling shape. normalize's own AST / CFG / scope / facts model is the feature
  source — no external annotation needed.
- **Label = the form chosen** at each site.
- **Internal nodes = context conditions; leaves = conditional conventions.**
- **Leaf purity = strictness** of the rule at that context.

**Information gain (entropy reduction)** is the mechanism — the same entropy meter,
made into an algorithm. The tree decomposes variance:

```
context-explained variance  →  conditional rule  →  collapse
residual leaf impurity       →  genuine decision  →  surface (decision stream)
```

This resolves the flat-marginal ambiguity cleanly:

- Flat marginal **but pure under context** → a conditional rule (e.g. "short-form
  when inside a closure body; long-form at top-level").
- Flat **and stays impure at every leaf** → a real free decision — surface to the
  decision stream for human resolution.

The decision stream = the variance no structural context can predict. That is
precisely the set of choices that are *genuinely underdetermined* by the codebase.

---

## Output Compiles to the Rule Engine

A decision tree is interpretable: each root-to-leaf path is a conjunction of
conditions, which is a stateable rule. The learned tree emits directly as
`normalize` fact/syntax rules, enforced and conformed via the refactor/edit engine.

**Conventions are learned AND legible, not hand-written, not a black box.**

The pipeline:

```
corpus instances  →  normalize-code-similarity (normalized AST hashing, bucketing)
                  →  decision-tree learner (per decision-class)
                  →  rule paths  →  normalize-rules / normalize-syntax-rules
                  →  normalize-refactor / normalize-edit (conform)
                  →  normalize-ratchet (hold the line)
```

Feature extraction draws on `normalize-cfg`, `normalize-scope`, and the facts
model — the same structural information already computed for other analyses.

---

## Why This Matters / Fit

This is the missing "where do canonical forms come from?" piece for the broader goal
of *collapsing the valid-program space*:

- **Exponential collapse**: constraining a constant *fraction* of choices (a whole
  decision-class) collapses the space exponentially — not a constant number of fixes.
- **Non-confluence is fine**: normal forms exist without uniqueness. The
  non-confluence IS the decisions. normalize need not produce a single canonical
  output to be useful.
- **Control flow is the highest-leverage class**: the most opinionated decisions
  with the most structural variation — and normalize's CFG/scope model already
  provides the features to explain that variation.

---

## Honest Bounds

1. **Ceiling is the feature model.** Only as good as the structural context exposed
   by normalize's AST/CFG/scope. A convention with no extractable feature to explain
   it will masquerade as a "free decision" (a false high-entropy leaf).

2. **MDL / pruning is mandatory.** Without it, the tree memorizes slop as spurious
   rules. Accept a split only if it *compresses* — the information gain must pay the
   description cost of the added node. MDL pruning is the false-oracle filter applied
   as tree regularization.

3. **Scoped to style.** Semantic canonicalization is excluded. The modal pattern can
   be the modal defect.

---

## See Also

- `normalize-code-similarity` — normalized AST hashing → instance bucketing per
  decision-class.
- `normalize-rules`, `normalize-syntax-rules`, `normalize-facts-rules-*` — compile
  target for learned rule paths.
- `normalize-cfg`, `normalize-scope` — feature source for structural context.
- `normalize-refactor`, `normalize-edit` — conform engine.
- `normalize-ratchet` — enforcement / hold-the-line.
- Companion essays in the rhi docs repo:
  `docs/decision-stream.md` @1d1ad10,
  `docs/deterministic-simulation-testing.md` @90a6350.
