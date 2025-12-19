# Codebase Tree Model

## Core Idea

The codebase is a single unified tree: filesystem + AST merged.

```
project/
  src/
    moss/
      dwim.py
        WORD_FORMS (dict)
        TFIDFIndex (class)
          add_document (method)
          query (method)
        resolve_tool (function)
        ToolRouter (class)
          analyze_intent (method)
      cli.py
        ...
  tests/
    ...
```

Every node is addressable. Navigation is spatial, not verb-based.

## Operations

1. **Point** - specify a location (fuzzy matching OK)
   - `src/moss/dwim` → file
   - `dwim.py:ToolRouter` → class
   - `resolve_tool` → function (search if ambiguous)

2. **View** - see that node + immediate children
   - Default: names + 1-line descriptions
   - Always shows "where you are" in context

3. **Zoom** - adjust what "children" means
   - Zoomed out: children = files/directories
   - Zoomed in: children = classes/functions/methods
   - Further: children = parameters, local variables

4. **Expand** - show more detail for a specific child
   - Full signature, docstring, body

## Questions

- How to represent depth? Implicit based on node type?
- How to handle cross-cutting concerns (callers, imports)?
- What's the default view when you point at a directory vs file vs symbol?

## UX Design

### Addressing nodes

Fuzzy, forgiving. All of these could work:
- `src/moss/dwim.py` - exact path
- `dwim.py` - filename (resolve if unique)
- `dwim` - stem
- `ToolRouter` - symbol name
- `dwim:ToolRouter` - scoped symbol
- `ToolRouter.analyze_intent` - dotted path

### What you see

Default view of any node:
```
[context: where this node lives]
[the node itself: name + description]
[children: names + 1-line descriptions]
```

Example: `moss view dwim.py`
```
src/moss/dwim.py (in moss/)

DWIM (Do What I Mean) - Semantic tool routing for LLM usage.

Classes:
  TFIDFIndex      TF-IDF index for semantic similarity
  ToolInfo        Information about a tool for semantic matching
  ToolMatch       Result of matching a query to a tool
  ToolRouter      Smart tool router using TF-IDF cosine similarity

Functions:
  resolve_tool    Resolve a tool name to its canonical form
  analyze_intent  Find best matching tools for a query
  suggest_tool    Suggest the best tool with confidence scoring
  ... +15 more
```

### Drilling down

`moss view ToolRouter` (or `moss view dwim:ToolRouter`):
```
src/moss/dwim.py > ToolRouter (in dwim.py)

Smart tool router using TF-IDF cosine similarity.

Methods:
  __init__        Initialize the router with tool descriptions
  analyze_intent  Analyze a query to find matching tools
  suggest_tool    Suggest the best tool for a query
```

### Expanding

`moss expand ToolRouter.analyze_intent`:
```
def analyze_intent(
    self,
    query: str,
    available_tools: Sequence[str] | None = None
) -> list[ToolMatch]:
    """Analyze a natural language query to find the best matching tools.

    Args:
        query: Natural language description of what the user wants
        available_tools: Limit search to these tools (default: all)

    Returns:
        List of ToolMatch sorted by confidence (highest first)
    """
    ...
```

### Discovery

`moss path <fuzzy>` - resolve to exact location(s):
```
$ moss path toolrouter
src/moss/dwim.py:ToolRouter (class)
tests/test_dwim.py:TestToolRouter (class)
```

`moss search <query> [scope]` - find matching nodes:
```
$ moss search "intent" src/moss
src/moss/dwim.py:analyze_intent (function)
src/moss/dwim.py:ToolRouter.analyze_intent (method)
src/moss/mcp_server.py:_dwim_rewrite (function) - "analyze intent"

$ moss search "get" ToolRouter
src/moss/dwim.py:ToolRouter._get_tfidf (method)
src/moss/dwim.py:ToolRouter._get_keywords (method)
```

Scope can be any node - directory, file, or symbol. Defaults to cwd.

### Navigation

- `moss parent <target>` - go up one level
- `moss siblings <target>` - other nodes at same level
- `moss callers <symbol>` - who references this
- `moss callees <symbol>` - what this references

### LLM-friendly commands

Natural language maps to tree operations:
- "what's in dwim.py" → `view dwim.py`
- "show me ToolRouter" → `view ToolRouter`
- "full code of analyze_intent" → `expand ToolRouter.analyze_intent`
- "what calls resolve_tool" → `callers resolve_tool`

## Implementation

### New commands (the whole interface)

| Command | Purpose |
|---------|---------|
| `moss view <target>` | Default view of any node |
| `moss expand <symbol>` | Full source |
| `moss path <fuzzy>` | Resolve to exact location(s) |
| `moss search <query> [scope]` | Find matching nodes |
| `moss callers <symbol>` | What references this |
| `moss callees <symbol>` | What this references |

That's it. Six commands.

### Tear out

- `skeleton` → replaced by `view` (file-level) + `expand` (symbol-level)
- `summarize` → replaced by `view`
- `anchors` → replaced by `search` with type filter
- `query` → replaced by `search`
- `tree` → replaced by `view` (directory-level)
- `context` → replaced by `view` + `callers`/`callees`
- `deps` → maybe keep for imports, or fold into `callees`?

### Keep (for now)

- `cfg` - control flow is different enough
- `complexity` - metrics are orthogonal
- `diff`, `pr`, `git-hotspots` - git stuff is separate domain

### Build order

1. Unified tree data structure (filesystem + AST)
2. `path` - fuzzy resolution (we have pieces of this)
3. `view` - default views at each level
4. `search` - scoped search
5. `expand` - full source (already have `skeleton_expand`)
6. `callers`/`callees` - reference tracking

## Open Questions

### Target maximum length / partial depth
If output would be huge, can we show "half" a level?
- 100 classes → show 5 representative ones + "... +95 more"
- Or: show classes but truncate method lists
- Needs heuristics for what's "representative"

### Indexing
Fast lookup needs an index. Options:
- In-memory: fast but rebuilds on each invocation
- SQLite: persistent, but adds complexity
- Simple JSON: symbol → (file, line) mapping
- Minimal index: just filenames + top-level symbols

Concern: avoid huge database. Maybe just index symbol names → locations,
not full AST. Rebuild incrementally on file changes (mtime check).

## Non-goals (for now)

- Editing through this interface
- Real-time updates
- Cross-repo navigation
