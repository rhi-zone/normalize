# Duplicate Detection: Design Notes

## What we have today

`analyze duplicate-functions` and `analyze duplicate-types` both use **exact structural hashing**:

- Walk the AST, hash each node kind recursively via `hash_node_recursive`
- `--elide-identifiers` (default on): ignore variable/function names → catches renamed clones
- `--elide-literals`: ignore string/number constants → catches copy-paste-with-tweaked-values
- Bucket by hash, report groups with ≥ 2 members

This is O(n) per file and precise — no false positives. But it only catches **Type 1 and Type 2 clones** (exact and renamed). A function with one added statement doesn't match.

## What's missing

### Subtree-level matching (exact)

Currently we hash function-rooted subtrees only. We could hash *all* subtrees above a size threshold:

```
for each node where subtree_size(node) >= N:
    hash = compute_function_hash(node, ...)
    bucket[hash].push(location)
```

This would find duplicate `if` blocks, loop bodies, argument lists, and class bodies — not just whole functions. Also catches "function A is a superset of function B" because the inner shared block matches even when the outer wrappers differ.

The main new work is **containment suppression**: if a 50-line match is found, suppress all its sub-matches (which trivially also match). Report only the largest matching subtrees.

### Partial/fuzzy matching (hard)

For subtrees that are *similar but not identical* — a few statements added, a variable renamed differently, a condition tweaked — exact hashing fails. Approaches:

**Token shingling + MinHash/LSH**
- Serialize each subtree to a token sequence (node kind ± leaf text)
- Compute k-shingles, estimate Jaccard similarity via MinHash
- LSH buckets candidate pairs without O(n²) pairwise comparison
- Good at: "mostly the same with scattered small changes"
- Used by: CPD-style tools

**Bag-of-subtrees / bag-of-nodes**
- Count frequency of each node kind (or small fixed-depth subtree shape) as a vector
- Cosine similarity between vectors
- Cheaper than shingling, loses structural order
- `a + b` and `b + a` look identical — acceptable depending on use case

**Tree edit distance**
- Exact similarity: minimum insert/delete/relabel operations to transform one tree to another
- O(n²–n³) per pair — only feasible after aggressive pre-filtering
- High accuracy, good for ranking/scoring after candidates are found

**Practical hybrid:**
1. Pre-filter with MinHash LSH to find candidate pairs above ~70% similarity
2. Score/rank candidates with TED for display

## What the real use cases are

Before building, it's worth asking: what problems are people actually trying to solve?

### "I have a lot of copy-paste" (most common)

The most common real-world motivation. Developer copies a function, tweaks a few lines, forgets to refactor. Over time the codebase has 6 versions of "process user input" with slight differences.

**What helps:** Exact subtree matching with elision handles most of this — it's usually rename + literal changes. Partial matching adds coverage for cases where a line was added/removed, but the signal degrades; at 60% similarity you're flooded with noise.

**Verdict:** Subtree-level exact matching (generalization of what we have) covers most real cases. Fuzzy matching has a poor signal-to-noise tradeoff for this use case.

### "I want to find refactoring opportunities"

Find places where an abstraction *could* be extracted — a recurring 15-line block across 8 functions that nobody noticed because they're spread across different files.

**What helps:** This is actually a strong argument *for* both subtree-level matching and partial matching. `analyze query` requires you to already know what pattern you're looking for. Clone detection is the discovery step: it tells you "this block appears in N places" without you having to suspect it first.

Exact matching with elision catches Type 1/2. But real-world copy-paste-and-tweak produces Type 3: someone copies a 15-line block, tweaks 2 lines in each copy. Exact matching misses it entirely — yet that's precisely the case where the pattern has *lived long enough to drift*, which is the strongest refactoring signal. The copies aren't identical because the codebase grew around them.

For this use case, false positives are cheap (you look and dismiss). The threshold can be tuned aggressively in a way it can't for CI enforcement.

**Verdict:** Strong use case for both subtree-level exact matching and partial/fuzzy matching. The discovery framing changes the noise calculus — exploration tolerates false positives that automated enforcement cannot.

### "I want to enforce DRY in code review"

Detect when a PR introduces code that already exists elsewhere. CI use case.

**What helps:** The existing `--diff` flag on `analyze duplicate-functions` already does this. The question is whether it catches enough. Fuzzy matching would help but with high false-positive risk in CI.

**Verdict:** Incremental improvement possible; full fuzzy matching probably too noisy for automated enforcement.

### "LLM context: avoid sending the same code twice"

When building context for an LLM, sending near-duplicate code wastes tokens and confuses the model. Deduplication before context assembly.

**What helps:** This is a different access pattern — you have a *candidate set* of code snippets and want to deduplicate them. MinHash/LSH or even simple token-overlap works well here since false positives are cheap (just skip a snippet) and the similarity threshold can be tuned aggressively.

**Verdict:** Good use case for fuzzy matching, but at the *context assembly layer* not the analysis layer. Probably belongs in normalize's agent/session tooling rather than `analyze`.

### "Find where a specific pattern is duplicated"

Developer knows there's duplication around, say, error handling or config parsing, but doesn't know where. Wants to find all instances.

**What helps:** `analyze query` with a tree-sitter pattern is more precise than clone detection here. Clone detection requires an existing instance to hash/match against.

**Verdict:** Not a strong fit for clone detection tools.

## Conclusion

**High value, low risk:** Generalize to subtree-level exact matching with containment suppression. Extends the current hash-per-function model naturally, same false-positive guarantees. This is the primary tool for surfacing non-obvious refactoring opportunities — patterns you didn't know existed.

**High value, higher complexity:** MinHash LSH for partial matching. The refactoring discovery use case — finding drifted copies — is a strong motivation, and the exploration context tolerates false positives. Also useful for LLM context-assembly deduplication. The two use cases have different access patterns (whole-codebase scan vs. candidate-set dedup) so may want separate surfaces.

**Not worth building:** Full tree edit distance scoring as a standalone feature. The O(n²–n³) cost only pays off as a ranking step after LSH pre-filtering, and even then the use case has to be clear before investing.
