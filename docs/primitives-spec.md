# Primitives Spec: view, edit, analyze

Three-tool interface for codebase navigation, modification, and analysis.

## view

Unified read operation. Shows nodes or lists matches.

```
normalize view <target> [options]
```

### Target Resolution

Fuzzy, forgiving:
- `crates/normalize-cli/src/skeleton.rs` - exact path
- `skeleton.rs` - filename
- `skeleton` - stem
- `SkeletonExtractor` - symbol name
- `skeleton.rs/SkeletonExtractor` - scoped

### Output Modes

- Single match → show content (structure + children)
- Multiple matches → list paths
- With filters → list matching paths

### Options

| Flag | Purpose |
|------|---------|
| `--type <type>` | Filter by node type: `file`, `class`, `function`, `method` |
| `--calls <symbol>` | Nodes that call symbol (callers) |
| `--called-by <symbol>` | Nodes that symbol calls (callees) |
| `--deps` | Include dependency information |
| `--depth <n>` | Expansion depth (default 1) |
| `--all` | Full depth expansion |

### Examples

```
normalize view src/foo.py              # show file structure
normalize view router                  # fuzzy → show ToolRouter
normalize view --type class            # list all classes
normalize view --calls resolve_tool    # what calls resolve_tool?
normalize view MyClass --deps          # show class with dependencies
normalize view src/ --type function    # functions in src/
```

## edit

Unified write operation. Modify nodes structurally.

```
normalize edit <target> <operation> [content]
```

### Operations

#### Delete
Remove a node entirely.
```
normalize edit src/foo.py/MyClass --delete
normalize edit src/foo.py/func --delete
```

#### Replace
Swap node content.
```
normalize edit src/foo.py/func --replace "def func(): return 2"
```

#### Insert (sibling-relative)
Insert before/after the target node.
```
normalize edit src/foo.py/MyClass --before "# Class comment"
normalize edit src/foo.py/MyClass --after "class Other: pass"
```

#### Insert (container-relative)
Insert as first/last child of target container.
```
normalize edit src/foo.py --prepend "import os"           # top of file
normalize edit src/foo.py --append "# EOF"                # end of file
normalize edit src/foo.py/MyClass --prepend "x = 1"       # first in class body
normalize edit src/foo.py/MyClass --append "def last(): pass"  # last in class body
```

#### Move
Cut node and insert at new location.
```
normalize edit src/foo.py/func --move-before src/bar.py/other
normalize edit src/foo.py/func --move-after src/bar.py/other
normalize edit src/foo.py/func --move-prepend src/bar.py/MyClass  # into class
normalize edit src/foo.py/func --move-append src/bar.py           # end of file
```

#### Copy
Copy node to new location (original remains).
```
normalize edit src/foo.py/func --copy-before src/bar.py/other
normalize edit src/foo.py/func --copy-after src/bar.py/other
normalize edit src/foo.py/func --copy-prepend src/bar.py/MyClass
normalize edit src/foo.py/func --copy-append src/bar.py
```

#### Swap
Exchange positions of two nodes.
```
normalize edit src/foo.py/func1 --swap src/foo.py/func2
```

### Special Cases

| Situation | Command | Behavior |
|-----------|---------|----------|
| Top of file | `edit file.py --prepend "import x"` | Before first node |
| End of file | `edit file.py --append "# EOF"` | After last node |
| Empty file | `edit file.py --append "# content"` | Creates first content |
| Empty class | `edit MyClass --append "pass"` | Adds to empty body |

### Error Handling

Strict matching (Claude Code style):
- Target must resolve unambiguously
- Fail fast on ambiguity, don't guess
- Clear error messages with resolution hints

### Content Format

Content is provided as a string. The tool:
- Parses content as code in the file's language
- Validates syntax before applying
- Preserves or adapts indentation to context
- Fails if content is syntactically invalid

## analyze

Unified analysis operation. Computes properties of codebase nodes.

```
normalize analyze [target] [options]
```

### Target Resolution

Same as `view` - fuzzy, forgiving paths.

### Analysis Types

| Flag | Purpose |
|------|---------|
| `--health` | Codebase health metrics (files, lines, avg complexity) |
| `--complexity` | Cyclomatic complexity per function |
| `--security` | Security vulnerability scanning |

Running with no flags runs all analyses.

### Examples

```
normalize analyze                       # full codebase analysis
normalize analyze src/                  # analyze src directory
normalize analyze --complexity          # just complexity
normalize analyze src/foo.py --security # security scan of one file
```

### Output

Returns structured results suitable for LLM consumption:
- Health: file count, line count, avg complexity score
- Complexity: list of functions with their complexity scores
- Security: list of findings with severity, location, description

## Open Questions

1. **Text-based fallback**: For non-structural edits (change a string literal), do we need `--text-replace old new`?

2. **Multi-edit**: Batch multiple operations? Or rely on multiple invocations?

3. **Dry-run**: `--dry-run` to preview changes without applying?

4. **Undo**: Track edit history for reversal? Or rely on git?

5. **Cross-file move**: `--move-*` across files - handle imports automatically?
