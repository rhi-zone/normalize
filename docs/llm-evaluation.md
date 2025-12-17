# LLM Introspection Tools Evaluation

This document captures findings from using Moss's introspection tools with Claude Code.

## Tools Evaluated

1. `moss skeleton` - Code structure extraction
2. `moss anchors` - Symbol finding
3. `moss query` - Pattern-based search
4. `moss cfg` - Control flow graphs
5. `moss deps` - Dependency extraction
6. `moss context` - Combined view

## Real-World Usage Session

### What I Tried

1. **Module overview with context**:
   ```bash
   moss context src/moss/policy.py
   ```
   Result: Excellent summary - shows 476 lines, 10 classes, 35 methods, imports/exports at a glance.

2. **Finding all exceptions**:
   ```bash
   moss query src/moss/ --inherits Exception --type class
   ```
   Result: Found 4 exception classes (GitError, AmbiguousAnchorError, AnchorNotFoundError, PatchError).

3. **Finding async methods**:
   ```bash
   moss query src/moss/ --signature "async def" --type method
   ```
   Result: Great for understanding async patterns - found all async methods with their signatures.

4. **Analyzing internal dependencies** (JSON + Python post-processing):
   ```bash
   moss --json deps src/moss/ | python3 -c "..."
   ```
   Result: Built a dependency graph showing which modules import which. Identified core modules.

5. **Finding largest modules**:
   ```bash
   moss --json skeleton src/moss/ | python3 -c "..."
   ```
   Result: memory.py (49 symbols), policy.py (46), api.py (42) are the largest.

6. **Docstring coverage analysis**:
   ```bash
   moss --json query src/moss/ --type method | python3 -c "..."
   ```
   Result: 71% of methods have docstrings (291/405).

### Key Observations

1. **JSON output is the killer feature** - Allows piping to Python for custom analysis
2. **Context command is the best starting point** - Gives just enough info to decide next steps
3. **Query command is very flexible** - Regex + type + inheritance filters cover most needs
4. **CFG is verbose** - Multiple functions with same name creates a lot of output

## What Works Well

### JSON Output
- The `--json` flag produces structured output that's easy to parse
- Consistent schema across commands
- Includes all relevant metadata (line numbers, signatures, docstrings)
- **Key insight**: Enables building custom analysis on top

### Context Command
- Provides a good "summary view" of a file
- Combines symbol count, imports, exports, and skeleton
- Useful for quick codebase orientation
- **Key insight**: Best first command for any file

### Query Command
- Flexible filtering by name, type, signature, and inheritance
- Regex support enables complex pattern matching
- Recursive search through directories
- **Key insight**: Most powerful for targeted searches

### Skeleton Extraction
- Captures class/function hierarchy well
- Preserves docstrings for understanding
- Signatures provide type information

### Deps Command
- Shows internal module relationships clearly
- Can build dependency graphs with post-processing
- **Key insight**: Critical for understanding architecture

## Areas for Improvement

### Missing Features
1. **Complexity metrics**: Line counts per function, cyclomatic complexity
2. **Reverse dependencies**: "Who imports this module?"
3. **Symbol size**: End line numbers to calculate function length
4. **Grouped output**: Option to organize by module for multi-file queries

### Output Refinements
1. **CFG verbosity control**: Option to show just structure, not all statements
2. **Configurable depth**: Show only top-level symbols vs full hierarchy
3. **Public API filter**: Show only `__all__` exports or non-underscore names
4. **Diff-friendly output**: For tracking changes over time

### UX Improvements
1. **Better error messages**: When no matches found, suggest alternatives
2. **Progress indicator**: For large directory scans
3. **Output paging**: Long outputs could be paginated

## Usage Patterns Discovered

### Effective Workflows

1. **Understanding a new file**:
   ```bash
   moss context <file>        # Get overview
   moss skeleton <file>       # See full structure
   ```

2. **Finding implementations**:
   ```bash
   moss query <dir> --inherits <base>   # Find subclasses
   moss query <dir> --signature <pattern>  # Find by signature
   ```

3. **Architecture analysis**:
   ```bash
   moss --json deps <dir> | python3 -c "..."  # Build dep graph
   moss --json skeleton <dir> | python3 -c "..."  # Count symbols
   ```

4. **Code quality checks**:
   ```bash
   moss --json query <dir> --type method | python3 -c "..."  # Docstring coverage
   ```

### LLM-Specific Considerations
- JSON output fits naturally into tool-use patterns
- Structured data reduces ambiguity in responses
- Symbol locations enable accurate code references
- Post-processing with Python is natural in LLM context

## Recommendations

### High Priority
1. **Add line counts**: `--min-lines`, `--max-lines` filters
2. **Add reverse deps**: "What modules import X?"
3. **Add symbol sizes**: Include end_line in all outputs

### Medium Priority
1. **CFG summary mode**: Just show node/edge counts, not full graph
2. **Public API filter**: `--public-only` flag
3. **Output grouping**: `--group-by=file` option

### Low Priority
1. **Incremental updates**: For large codebases
2. **Caching**: Speed up repeated queries
3. **Watch mode**: Real-time updates

## Test Coverage

All introspection commands have unit tests covering:
- Basic functionality
- JSON output format
- Error handling
- Filter combinations

656 tests currently passing with ~86% coverage.
