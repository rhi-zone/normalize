# LLM Code Consistency

Research notes on detecting and preventing code inconsistencies introduced by LLMs.

## Core Insight

**LLMs add inconsistencies because they don't see the full picture when writing each piece.**

When an LLM writes `cmd_complexity()`, then later writes `cmd_security()`, it doesn't remember the first. Each function is written in isolation, leading to drift in:
- Argument handling patterns
- Error handling approaches
- Output formatting
- Helper usage

This is fundamental to how LLMs work with limited context windows and session boundaries.

## Case Study: Moss CLI (2025-12-25)

Exploration of redundancy in Rust and Python CLIs revealed systemic inconsistencies.

### Rust CLI (`crates/normalize-cli/`)

| Issue | Scope | Notes |
|-------|-------|-------|
| File resolution boilerplate | 15+ commands | Same `path_resolve::resolve()` + error handling. **FIXED**: `resolve_and_read()` helper |
| JSON output formatting | 25+ commands | `if json { println!(serde_json::json!(...)) }` everywhere |
| Root directory resolution | All 33 commands | Same 3 lines at start of every command |
| Call graph auto-index check | callees/callers/find_symbols | Identical check-and-reindex logic |
| `callers.rs` ≈ `callees.rs` | 90% similar | Should share implementation, keep separate commands |
| `deps` vs `imports` | Overlapping | Different flags, same domain |
| `health`/`overview`/`analyze` | Fragmented | Three commands for related analysis |

### Python CLI (`packages/normalize-cli/`)

| Issue | Scope | Notes |
|-------|-------|-------|
| Directory validation | ~20 commands | Same Path resolve + exists check |
| Output format handling | ~29 commands | JSON/markdown/compact branching |
| MossAPI init + error handling | ~15 commands | try/except wrapper pattern |
| Comma-separated string parsing | ~8 commands | `[t.strip() for t in s.split(",")]` |
| Analysis command template | 5 commands | complexity/clones/security/patterns/weaknesses |
| Server command pattern | 4 commands | Import + run + KeyboardInterrupt handling |
| Async wrapper pattern | ~8 commands | `asyncio.run()` with error handling |
| `report` vs `overview` | Unclear distinction | Both analyze codebase health |
| `search` vs `rag` | Functional overlap | Both do semantic search |
| **Directory arg style** | Mixed | Some positional, some `--directory/-C` - egregious |
| Argument parser setup | Repeated | `--json`, directory args defined 20+ times |

### Severity Assessment

Most egregious (user-facing inconsistency):
- **Directory argument style** - positional in some commands, flag in others. Users can't build muscle memory.
  - **FIXED (2025-12-25)**: Standardized short flag to `-C` across all commands (was mix of `-C` and `-d`).

Most wasteful (code bloat):
- **Output format handling** - 29 commands × ~5 lines = 150 lines of near-identical code

Most subtle (hard to notice):
- **Error message inconsistency** - same error, different wording across commands

## Detection Approaches

### What we have
- `moss clones` - textual clone detection

### What we could build
- **Structural clone detection** - same AST shape, different identifiers
- **Function signature clustering** - group by `(arg_count, return_type, imports_used)`
- **Call pattern fingerprinting** - functions that call same sequence
- **Argument pattern analysis** - detect when CLI commands diverge in arg style

### Key metrics
- Jaccard similarity of function bodies (token-level)
- Edit distance between ASTs
- Import set overlap
- Call graph neighborhood similarity

## Prevention Approaches

### Pre-generation
- "Before writing `cmd_foo()`, here's how similar commands are structured"
- Pattern extraction: automatically derive templates from repeated code
- Show 2-3 exemplars of the pattern before LLM writes new instance

### Post-generation
- Structural diff: "your new function differs from the pattern in these ways"
- Consistency linting: flag deviations from established patterns
- PR-level analysis: "this PR introduces a new pattern for X, but Y existing files do it differently"

### Process
- Pattern registry that LLMs must consult
- Pre-flight hooks that inject relevant examples
- Session handoff notes that capture established patterns

## Open Questions

1. How to surface patterns without overwhelming context?
2. Can we detect "pattern emergence" - when 3+ things become similar enough to extract?
3. Should prevention be advisory (suggest) or enforced (reject)?
4. How to handle legitimate variation vs accidental inconsistency?

## Related

- `docs/philosophy.md` - "Generalize Don't Multiply" principle
- Clone detection literature (CCFinder, SourcererCC, etc.)
