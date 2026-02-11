# Retrospective: ast-grep Integration Implementation

## Summary

Adding ast-grep pattern support to `normalize analyze query` took ~30 turns when it could have been done in ~10. This document analyzes why.

## What Was Built

- Updated tree-sitter 0.25 → 0.26
- Added `ast-grep-core` dependency
- Created `DynLang` adapter (implements ast-grep's `Language` + `LanguageExt` traits for dynamic grammars)
- Rewrote `cmd_query` to support both S-expr and ast-grep patterns
- Made `[PATH]` optional for multi-file search
- Added proper `OutputFormat` support (--json, --jq, --pretty, --compact)
- Added tree-sitter syntax highlighting for matches

## What Went Wrong

### 1. Didn't Check Existing Patterns Before Starting

**Problem:** Started implementing output as `json: bool` parameter, then had to refactor to `OutputFormat`. Then forgot --jq. Then forgot --pretty. Then used naive yellow coloring instead of `highlight_source`.

**What I should have done:** Before writing any code, check how similar commands handle output:
```bash
grep -rn "OutputFormat\|highlight_source" crates/normalize/src/commands
```

This would have revealed:
- Commands receive `OutputFormat`, not `json: bool`
- `OutputFormat::Jq(filter)` exists and needs handling
- `tree::highlight_source()` is the standard way to color code
- Pretty mode means syntax highlighting, not just "use colors"

### 2. Trial-and-Error with ast-grep-core API

**Problem:** Spent many turns figuring out ast-grep-core's API:
- First tried `Pattern::new()` - wrong
- Then `Pattern::try_new()` - right
- Tried `doc.root()` - doesn't exist on StrDoc
- Then `lang.ast_grep(source).root()` - correct
- Tried destructuring `get_matched_variables()` as tuples - wrong, it returns `MetaVariable` enum
- `range()` returns `Range<usize>` (bytes), not position struct

**What I should have done:**
- Read the ast-grep source more carefully upfront
- Create a minimal test file first to validate API understanding before integrating
- The test code I wrote in `/tmp/ast-grep-test` was useful but came too late

### 3. Didn't Anticipate tree-sitter Version Mismatch

**Problem:** ast-grep-core uses tree-sitter 0.26, we used 0.25. Had to update and fix 5 call sites where `child(usize)` became `child(u32)`.

**What I should have done:** Check dependency versions before adding:
```bash
cargo tree -p ast-grep-core | grep tree-sitter
```

### 4. Incremental "Fix Compile Error" Loop

**Problem:** Made changes, ran `cargo check`, fixed one error, repeat. This is slow and misses the bigger picture.

**What I should have done:**
- Plan the full function signature changes upfront
- Trace through all call sites before making changes
- Make all related changes in one pass

### 5. Forgot to Track Grammar Name for Highlighting

**Problem:** Added syntax highlighting but hadn't stored the grammar name in `MatchResult`. Had to add it and update all constructors.

**What I should have done:** When adding a feature that needs per-result metadata, think about what data is needed upfront.

## Time Distribution (Estimated)

| Activity | Turns | Should Have Been |
|----------|-------|------------------|
| Initial discussion/planning | 2 | 2 |
| tree-sitter version update | 3 | 1 |
| ast-grep-core API discovery | 8 | 2 |
| Basic implementation | 5 | 3 |
| OutputFormat refactoring | 4 | 0 (should have been right first time) |
| --jq support | 2 | 0 (should have been included) |
| --pretty / highlighting | 4 | 1 |
| Fixing grammar name for highlighting | 2 | 0 |
| **Total** | **~30** | **~9** |

## Lessons for CLAUDE.md

1. **Before implementing output, check existing patterns:**
   - Search for `OutputFormat` usage in similar commands
   - Check for `highlight_source` usage
   - Look at how --json/--jq/--pretty are handled elsewhere

2. **Before adding dependencies, check version compatibility:**
   - `cargo tree -p <new-dep>` to see transitive deps
   - Compare versions with existing deps

3. **When integrating unfamiliar crates:**
   - Write a minimal standalone test first
   - Read the crate's test files for usage examples
   - Check if there's a `Language` or similar trait that needs implementing

4. **Plan data structures upfront:**
   - What fields will result types need?
   - What metadata is needed for output formatting?

## What Went Right

- Correctly identified that both S-expr and ast-grep patterns should be supported
- Auto-detection via `starts_with('(')` is simple and effective
- The `DynLang` adapter design is clean
- Multi-file search with grammar grouping is efficient
- Final output is consistent with rest of normalize

## Suggested CLAUDE.md Addition

```markdown
## Before Implementing CLI Output

1. Check how similar commands format output:
   - Do they use `OutputFormat` or raw `json: bool`?
   - Do they handle `--jq`?
   - Do they use `highlight_source` for code?

2. Search for patterns:
   ```bash
   grep -rn "OutputFormat\|highlight_source\|format.is_json" crates/normalize/src/commands
   ```

3. Result structs should include metadata needed for formatting (e.g., grammar name for syntax highlighting)
```
