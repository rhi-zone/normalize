# LLM Introspection Tools Evaluation

Findings from using Normalize's introspection tools.

## DWIM Effectiveness

| Feature | Accuracy | Confidence | Notes |
|---------|----------|------------|-------|
| Typo correction | 100% | 0.80-0.93 | "skelton" → "skeleton" works |
| Alias resolution | 100% | 1.00 | "imports" → "deps" perfect |
| Natural language | 100% top-3 | 0.24-0.51 | Correct tool found, but confidence below threshold |

**Issue**: Natural language queries have low confidence despite correct results. The TF-IDF approach works for ranking but confidence scores don't reflect accuracy.

**Recommendation**: Lower `SUGGEST_THRESHOLD` from 0.5 to 0.3, or use top-k results regardless of threshold.

## Tool Effectiveness

### What Works Well

**context** — Best entry point for any file
- Shows lines, symbol counts, imports at a glance
- Good for deciding what to explore next

**query --inherits** — Finding subclasses
- `normalize query src/ --inherits Exception --type class` found all 4 exception classes instantly
- Much faster than grep for semantic queries

**JSON + Python** — Custom analysis
- `normalize --json deps src/ | python3 -c "..."` enables arbitrary analysis
- Built dependency graph, found most-imported modules

**skeleton** — Code structure
- 19 top-level symbols in dwim.py identified correctly
- Signatures and docstrings preserved

### Gaps

1. **No line counts per function** — Can't filter by complexity
2. **No reverse deps** — "What imports this module?" not directly available
3. **No symbol sizes** — End line numbers would help estimate function length
4. **CFG verbosity** — Full graph output overwhelming for large functions

## Usage Patterns

**Understanding a file**: `normalize context <file>`

**Finding implementations**: `normalize query <dir> --inherits <base>` or `--signature <pattern>`

**Dependency analysis**: `normalize --json deps <dir> | python3 -c "..."`

**Symbol inventory**: `normalize --json skeleton <dir> | python3 -c "..."`

## Test Results

```
DWIM:
- Typo correction: 7/7 (100%)
- Alias resolution: 8/8 (100%)
- NL routing top-3: 8/8 (100%)

Tools on Normalize codebase:
- context: Shows 596 lines, 4 classes, 15 functions, 5 methods for dwim.py
- query --inherits Exception: Found 4 exception classes
- deps analysis: Identified normalize.views (7 imports) as most-used internal module
- skeleton: Extracted 19 top-level symbols correctly
```

## Recommendations

### High Priority
1. Lower DWIM threshold for natural language
2. Add line counts for complexity filtering

### Medium Priority
1. Add reverse dependency lookup
2. Add symbol end lines for size calculation

### Low Priority
1. CFG summary mode
2. Grouped multi-file output
