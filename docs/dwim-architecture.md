# DWIM Architecture

This document describes the DWIM (Do What I Mean) module internals.

## Overview

With the consolidation to 3 core primitives (view, edit, analyze), DWIM has been greatly simplified. The complex TF-IDF and embedding-based routing has been removed in favor of simple exact/alias matching with typo correction.

## Core Primitives

```python
CORE_PRIMITIVES = {"view", "edit", "analyze"}
```

These 3 tools are the primary CLI/MCP interface:
- `view` - Show tree, file skeleton, or symbol source (fuzzy path resolution)
- `edit` - Structural code modifications
- `analyze` - Health, complexity, and security analysis

## Aliases

Common terms map directly to primitives:

```python
CORE_ALIASES = {
    # view aliases
    "show": "view", "look": "view", "skeleton": "view", "tree": "view",
    "expand": "view", "search": "view", "find": "view", "grep": "view",

    # edit aliases
    "modify": "edit", "change": "edit", "patch": "edit", "fix": "edit",

    # analyze aliases
    "check": "analyze", "health": "analyze", "complexity": "analyze",
    "security": "analyze", "lint": "analyze",
}
```

## Resolution Logic

`resolve_core_primitive(name: str) -> tuple[str | None, float]`

1. **Exact match**: Check if name is in `CORE_PRIMITIVES` → confidence 1.0
2. **Alias match**: Check if name is in `CORE_ALIASES` → confidence 1.0
3. **Typo correction**: Use Levenshtein distance (SequenceMatcher)
   - Threshold 0.7 for auto-correct (e.g., "veiw" → "view")
   - Returns best match with confidence score

## Path Resolution

Handled by Rust CLI, not DWIM. The `view` and `analyze` commands accept fuzzy paths:

```
dwim.py       → src/moss/dwim.py
ToolRouter    → src/moss/dwim.py/ToolRouter
src/foo       → src/foo.py or src/foo/
```

Resolution order:
1. Exact path match
2. Filename match (stem.py)
3. Stem match (no extension)
4. Symbol search

## Legacy Support

The old TOOL_REGISTRY with ~30 tools still exists for backwards compatibility but is no longer the primary routing mechanism. New code should use the 3 primitives directly.

## Testing

Tests in `tests/test_dwim.py`:
- Core primitive resolution
- Alias resolution
- Typo correction thresholds
- Edge cases (empty, unknown)

## Configuration

Users can add custom aliases via `.moss/dwim.toml`:

```toml
[aliases]
ll = "view"      # shortcut
refs = "view"    # semantic alias
```

## Performance

With only 3 primitives + aliases, routing is O(1) lookup. Typo correction is O(n) where n = number of aliases (~30), negligible.
