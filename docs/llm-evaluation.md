# LLM Introspection Tools Evaluation

This document captures findings from using Moss's introspection tools with Claude Code.

## Tools Evaluated

1. `moss skeleton` - Code structure extraction
2. `moss anchors` - Symbol finding
3. `moss query` - Pattern-based search
4. `moss cfg` - Control flow graphs
5. `moss deps` - Dependency extraction
6. `moss context` - Combined view

## What Works Well

### JSON Output
- The `--json` flag produces structured output that's easy to parse
- Consistent schema across commands
- Includes all relevant metadata (line numbers, signatures, docstrings)

### Context Command
- Provides a good "summary view" of a file
- Combines symbol count, imports, exports, and skeleton
- Useful for quick codebase orientation

### Query Command
- Flexible filtering by name, type, signature, and inheritance
- Regex support enables complex pattern matching
- Recursive search through directories

### Skeleton Extraction
- Captures class/function hierarchy well
- Preserves docstrings for understanding
- Signatures provide type information

## Areas for Improvement

### Missing Features
1. **Complexity metrics**: Line counts, cyclomatic complexity would help prioritize review
2. **Cross-module view**: Seeing which modules depend on which would aid navigation
3. **Semantic search**: Embedding-based search for conceptual queries

### Output Refinements
1. **Configurable verbosity**: Sometimes less detail is better
2. **Focused views**: Extract just public API vs all symbols
3. **Diff-friendly output**: For tracking changes over time

## Usage Patterns Observed

### Effective Patterns
- Use `moss context` first to understand a file
- Use `moss query --inherits X` to find subclasses
- Use `moss deps` to trace import chains
- Use `moss skeleton` for directory overview

### LLM-Specific Considerations
- JSON output fits naturally into tool-use patterns
- Structured data reduces ambiguity in responses
- Symbol locations enable accurate code references

## Recommendations

1. **Keep JSON as primary output** - Most useful for programmatic interaction
2. **Add complexity filtering** - `--min-lines`, `--min-complexity`
3. **Consider incremental updates** - For large codebases, delta views
4. **Integrate with MCP** - Direct tool access without CLI overhead

## Test Coverage

All introspection commands have unit tests covering:
- Basic functionality
- JSON output format
- Error handling
- Filter combinations

656 tests currently passing with ~86% coverage.
