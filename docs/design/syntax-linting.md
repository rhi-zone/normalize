# Syntax-Based Linting Design

Custom rules using tree-sitter queries, like ESLint's `no-restricted-syntax`.

## Problem

Some patterns should be restricted to specific locations:
- `GrammarLoader::new` should only appear in `grammar_loader()` singleton
- `Query::new` should only appear in cached query getters
- Direct `unwrap()` on user input should be flagged

Currently no way to express these rules without external tools (semgrep, etc).

## Solution

Tree-sitter query-based rules with location allowlists.

### Rule Definition

Rules in `.moss/rules/` as `.scm` files with TOML frontmatter:

```scm
# .moss/rules/no-grammar-loader-new.scm
# ---
# id = "no-grammar-loader-new"
# severity = "error"
# message = "Use grammar_loader() singleton instead of GrammarLoader::new"
# allow = ["**/grammar_loader.rs"]
# ---

(call_expression
  function: (scoped_identifier
    path: (identifier) @_type
    name: (identifier) @_method)
  (#eq? @_type "GrammarLoader")
  (#eq? @_method "new")) @match
```

### CLI Interface

```bash
# Run all rules
moss analyze rules

# Run specific rule
moss analyze rules --rule no-grammar-loader-new

# List available rules
moss analyze rules --list

# Authoring helpers
moss analyze ast <file>           # Show AST for a file
moss analyze ast <file> --at 42   # Show AST node at line 42
moss analyze query <file> <query> # Test query against file
```

### Authoring Tools

To help write queries, expose AST inspection:

```bash
# Dump full AST with node types
$ moss analyze ast src/main.rs
(source_file
  (function_item
    name: (identifier) "main"
    body: (block
      (expression_statement
        (call_expression ...)))))

# Show node at cursor/line
$ moss analyze ast src/main.rs --at 42
Line 42 is inside:
  call_expression (L42:5-42:30)
    function: scoped_identifier (L42:5-42:22)
      path: identifier "GrammarLoader" (L42:5-42:18)
      name: identifier "new" (L42:20-42:22)
    arguments: arguments (L42:23-42:30)

# Test a query interactively
$ moss analyze query src/main.rs '(call_expression function: (scoped_identifier) @fn)'
3 matches:
  src/main.rs:42 - GrammarLoader::new()
  src/main.rs:55 - Config::load()
  src/main.rs:78 - Parser::parse()
```

### Rule Storage

1. **Project rules**: `.moss/rules/*.scm`
2. **Global rules**: `~/.config/moss/rules/*.scm`
3. **Builtin rules**: Compiled into moss (optional, curated set)

### Configuration

In `.moss/config.toml`:

```toml
[rules]
# Enable/disable specific rules
enabled = ["no-grammar-loader-new", "no-unwrap-on-input"]
disabled = ["some-noisy-rule"]

# Severity overrides
[rules.severity]
no-grammar-loader-new = "warn"  # downgrade from error
```

### Implementation

1. **Rule loader**: Parse `.scm` files with TOML frontmatter
2. **Query runner**: Execute tree-sitter queries against files
3. **Match filter**: Apply allowlist patterns to matches
4. **Reporter**: Format findings (text, JSON, SARIF)

Core types:

```rust
pub struct Rule {
    id: String,
    query: tree_sitter::Query,
    severity: Severity,
    message: String,
    allow: Vec<glob::Pattern>,
}

pub struct Finding {
    rule_id: String,
    file: PathBuf,
    range: Range,
    message: String,
    severity: Severity,
}
```

### Query Language

Use tree-sitter's native S-expression query syntax:
- Pattern matching: `(function_item name: (identifier) @name)`
- Predicates: `#eq?`, `#match?`, `#any-of?`
- Captures: `@name`, `@match` (special: marks the finding location)

The `@match` capture is required and marks where the finding is reported.

### Phase 1: MVP

1. `moss analyze ast <file>` - dump AST
2. `moss analyze query <file> <query>` - test queries
3. Rule files with basic format
4. `moss analyze rules` - run all rules

### Phase 2: Polish

1. `--at <line>` for focused AST inspection
2. Allow patterns (glob-based)
3. Severity configuration
4. SARIF output for IDE integration

### Phase 3: Ecosystem

1. Builtin rule library
2. Rule sharing/import mechanism
3. Auto-fix support (where possible)

## Non-Goals

- Semantic analysis (type information, control flow)
- Cross-file analysis (imports, call graphs)
- Auto-fix for complex patterns

These would require deeper integration with the indexer.

## Alternatives Considered

### Semgrep/ruff integration
Pro: Mature, feature-rich
Con: External dependency, different query syntax, overkill for simple patterns

### Custom DSL
Pro: Tailored to our needs
Con: Yet another language to learn, maintenance burden

### Tree-sitter queries (chosen)
Pro: Standard syntax, reusable knowledge, extensive documentation
Con: Learning curve for users unfamiliar with S-expressions

## Success Criteria

- Can express "X only allowed in Y" rules
- Authoring workflow is discoverable (`analyze ast`, `analyze query`)
- Performance: <100ms for typical project
- Rules are portable (share in repos, copy between projects)
