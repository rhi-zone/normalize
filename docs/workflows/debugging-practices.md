# Debugging Practices

Cross-cutting practices that support effective debugging across all workflows. These are patterns learned from real debugging sessions, not theoretical best practices.

## The Issues Log

Maintain a running log of issues encountered and resolved. This is project-specific tribal knowledge that prevents rediscovering the same bugs.

### Format

```markdown
# Issues Log

## YYYY-MM-DD

### [FIXED] Brief Description

**Symptom**: How it manifested (what you observed)

**Root Cause**: Why it happened (the actual bug)

**Fix**: How it was resolved (code changes)

**Files**: Which files were modified

**Prevention**: How to avoid similar issues in the future
```

### Why Each Section Matters

| Section | Purpose |
|---------|---------|
| **Symptom** | Pattern matching - "I've seen this before" |
| **Root Cause** | Understanding, not just fixing |
| **Fix** | Reference for similar fixes |
| **Prevention** | The actual value - stops recurrence |

### Example Entry

```markdown
### [FIXED] Blurry Output - Missing Input Scaling

**Symptom**: Generated images were extremely blurry compared to reference.

**Root Cause**: k-diffusion formulation requires input variance normalization.
The model expects input with variance ~1.0. Without normalization, inputs
have wrong magnitude.

Formula: `c_in = 1 / sqrt(1 + sigma^2)`

**Fix**: Apply c_in scaling before each forward pass:
```rust
let c_in = 1.0 / (1.0 + sigma * sigma).sqrt();
let input_scaled = input.clone() * c_in;
let output = model.forward(input_scaled, t, cond);
```

**Files**: `src/pipeline.rs`

**Prevention**:
- When implementing from papers, document expected input/output formulation
- k-diffusion vs diffusers vs paper formulations differ - be explicit
- Test: Compare intermediate values against reference implementation
```

### The Prevention Section

This is the most valuable part. Good prevention entries:

```markdown
**Prevention**:
- Validate model structure against reference before hardcoding defaults
- Test: Load weights, check all layers have matching shapes
- Could catch with: `expected_weights()` function that validates required tensors
```

Bad prevention entries:

```markdown
**Prevention**:
- Be more careful
- Test better
```

Prevention should be **actionable** and **specific**.

### Issues Log as Project-Specific CLAUDE.md

The issues log is essentially behavioral rules derived from pain:
- CLAUDE.md: "Don't do X" (general)
- Issues log: "We did X, here's what broke, here's why" (specific)

Both serve the same purpose: encode knowledge so it's not rediscovered.

## Build Debug Tooling

When you encounter a category of bug, build tooling to catch it faster next time.

### Example: NaN Detection

After debugging f16 overflow issues:

```rust
// Add --debug nan flag
if debug_flags.contains("nan") {
    check_tensor(&tensor, "step_0_noise");
}

fn check_tensor(t: &Tensor, name: &str) {
    let data = t.to_data();
    let nan_count = data.iter().filter(|x| x.is_nan()).count();
    let inf_count = data.iter().filter(|x| x.is_infinite()).count();

    if nan_count > 0 || inf_count > 0 {
        panic!("[NaN check failed] {}: {}/{} NaN, {}/{} Inf",
            name, nan_count, data.len(), inf_count, data.len());
    }
}
```

### Debug Tooling Patterns

| Bug Category | Tooling |
|-------------|---------|
| Numeric instability | NaN/Inf checks at key points |
| Shape mismatches | Shape logging before operations |
| Timing issues | Instrumentation, timestamps |
| State corruption | Snapshot/diff capabilities |
| Non-determinism | Seed control, determinism mode |

### When to Build vs When to Debug

Build tooling when:
- You've hit this category of bug 2+ times
- The bug is hard to localize
- Others might hit the same bug

Just debug when:
- One-off issue
- Quick to diagnose
- Won't recur

## Reference Implementation Comparison

Many bugs come from subtle differences vs reference implementations.

### Enumerate All Layers

Before implementing:

```bash
# List all weight names in reference model
python -c "
import safetensors
with safetensors.safe_open('model.safetensors', framework='pt') as f:
    for name in sorted(f.keys()):
        print(name)
" > expected_weights.txt
```

After implementing:

```bash
# Compare against what we actually load
diff expected_weights.txt loaded_weights.txt
```

### Compare Intermediate Values

```python
# Reference implementation
ref_output = reference_model(input)
print(f"Reference: min={ref_output.min()}, max={ref_output.max()}, mean={ref_output.mean()}")

# Our implementation
our_output = our_model(input)
print(f"Ours: min={our_output.min()}, max={our_output.max()}, mean={our_output.mean()}")

# Diff
diff = (ref_output - our_output).abs()
print(f"Max diff: {diff.max()}, Mean diff: {diff.mean()}")
```

### Formulation Mismatches

Different implementations use different formulations:
- k-diffusion vs diffusers vs original paper
- Biased vs unbiased variance
- Channel-first vs channel-last

Document which formulation you're using and why.

## Golden Tests

Capture known-good outputs and compare against them.

### Types of Golden Tests

| Test Type | Purpose |
|-----------|---------|
| **Exact match** | Deterministic components |
| **Statistical match** | Stochastic components (within tolerance) |
| **Structural match** | Output has right shape/type |
| **Regression** | Output doesn't get worse |

### Determinism Requirements

For golden tests to work, you need determinism:

```python
# Bad: Non-deterministic
vocab = {token: idx for idx, token in enumerate(hashmap.values())}

# Good: Deterministic
tokens = sorted(hashmap.values())
vocab = {token: idx for idx, token in enumerate(tokens)}
```

Common determinism killers:
- HashMap/HashSet iteration
- Parallel execution order
- Floating point accumulation order
- Unseeded random

### Golden Test Structure

```python
def test_tokenizer_golden():
    """Golden test: tokenizer produces known-good token IDs."""
    input_text = "a cute cat"
    expected_ids = [320, 2242, 2368]  # Known from reference

    actual_ids = tokenizer.encode(input_text)

    assert actual_ids == expected_ids, f"Expected {expected_ids}, got {actual_ids}"
```

## The Tool Output Problem

From agent debugging observations:

> Agent succeeds when tool output = answer, struggles when output requires interpretation/assembly across many pieces.

### Implications for Debug Tooling

Good debug output:
```
[NaN check failed] step_3_noise: 1234/65536 values are NaN
First NaN at index 4096 (attention layer output)
Previous step was clean - NaN introduced in attention
```

Bad debug output:
```
Step 3 complete.
Tensor stats: various values
Check logs for details.
```

The good output **is** the diagnosis. The bad output requires interpretation.

### Design for Diagnosis

When building debug tooling, ask:
- Does the output tell me what's wrong?
- Or does it give me data to analyze?

Prefer the former. The LLM (or human) shouldn't have to assemble clues.

## Testing Strategy Derived from Bugs

After accumulating issues, patterns emerge. From a real issues log:

```markdown
## Testing Strategy (Derived from Issues)

These issues suggest the following testing priorities:

1. **Golden tests with reference outputs**: Compare to PyTorch on identical inputs
2. **Weight completeness validation**: Assert all expected weights are loaded
3. **Determinism tests**: Run twice, outputs must be identical
4. **Precision-specific tests**: Test f16 and f32 separately
5. **Numeric stability tests**: Check for NaN/inf in intermediate tensors
```

This is valuable - the testing strategy emerges from actual failures, not theoretical coverage.

## Multi-Step Method Debugging

When debugging multi-step algorithms (samplers, iterative refinement):

### Isolate the Step

```
First order works, second order blurry
→ Core math correct, multi-step correction wrong
→ Focus on: history storage, coefficients, first-step handling
```

### Common Multi-Step Bugs

| Symptom | Likely Cause |
|---------|--------------|
| Works order 1, fails order 2+ | History/coefficient bug |
| Gradual degradation | Accumulating error |
| Sudden failure at step N | State corruption |
| Non-deterministic | Race condition in state |

### Debugging Approach

1. Verify single-step correctness (compare to reference)
2. Verify state is stored correctly between steps
3. Verify coefficients match reference
4. Check edge cases (first step, last step)

## When to Skip vs When to Debug

Sometimes the fix is "don't build it this way" rather than "debug harder."

### Signs to Skip

- Debugging longer than implementation would take
- Root cause is architectural, not a bug
- Multiple interacting bugs (fix one, another appears)
- Requirements have changed, making current approach obsolete

### Signs to Continue

- Root cause is localizable
- Similar code works elsewhere
- Fix is valuable beyond immediate need
- You're learning something transferable

### The Skip Decision

```markdown
**Decision**: Skip and redesign

**Rationale**:
- Ephemeral object lifecycle depends on 5 subsystems
- Each subsystem has potential bugs
- Fixing one reveals another
- Architecture doesn't support the requirement cleanly

**Action**: Move to architecture redesign, revisit later
```

Document the skip. It's not a failure, it's a decision.

## Failure Modes of LLM Debugging

Where LLM-assisted debugging struggles:

| Failure Mode | Why | Mitigation |
|--------------|-----|------------|
| Too many moving parts | Can't hold full system in context | Isolate subsystems |
| Subtle math differences | Requires deep domain knowledge | Reference implementation comparison |
| Non-reproducible | Can't observe the bug | Add logging, determinism |
| Output "looks wrong" | Subjective, hard to specify | Golden tests with known-good output |
| Cascading failures | Fix one, break another | Atomic changes, better tests |

### The Token Burn Pattern

```
LLM tries fix A → doesn't work
LLM tries fix B → doesn't work
LLM tries fix C → doesn't work
... (many tokens later)
Human: "actually the bug is in subsystem X"
```

Mitigation: Invest in diagnosis before fixes. The issues log helps here - pattern match against known symptoms.

## See Also

- [Bug Investigation](bug-investigation.md) - Finding the bug
- [Bug Fix](bug-fix.md) - Fixing the bug
- [Flaky Test Debugging](flaky-test-debugging.md) - Non-deterministic failures
- [Performance Regression Hunting](performance-regression-hunting.md) - "It got slow"

