Tree-sitter grammar for Jinja2 templates, written from scratch.

Replaces the flat `arborium-jinja2` grammar (which models all statements as
`statement + keyword`) with a grammar that has distinct named node types for every
statement kind and a full expression language.

**Statement types**: extends_statement, import_statement, from_statement,
include_statement, block_statement, macro_statement, for_statement, if_statement,
call_statement, filter_block_statement, set_statement, set_block_statement,
with_statement, autoescape_statement, trans_statement, do_statement, debug_statement,
raw_statement.

**Expressions**: full binary/unary operator hierarchy, filter expressions, test
expressions, call/attribute/subscript expressions, list/dict/tuple literals.

**External scanner** (`src/scanner.c`): handles `content` (literal text between tags)
and `raw_content` (opaque text inside `{% raw %}...{% endraw %}`).

**Tests**: 54 corpus tests in `test/corpus/` (expressions.txt, statements.txt).

**Files**:
- `grammar.js` — grammar source
- `tree-sitter.json` — grammar metadata
- `src/parser.c` — generated parser (do not edit; run `tree-sitter generate`)
- `src/scanner.c` — external scanner (edit directly)
- `src/node-types.json` — generated node type metadata
