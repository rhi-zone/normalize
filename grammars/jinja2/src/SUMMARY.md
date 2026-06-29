Generated and hand-written source files for the Jinja2 tree-sitter grammar. Do not edit generated files (`parser.c`, `node-types.json`) directly — regenerate via `tree-sitter generate` from `grammar.js` at the repo root.
- `parser.c` — generated LALR parser.
- `scanner.c` — external scanner (hand-written); handles `content` (literal text between tags) and `raw_content` (inside `{% raw %}...{% endraw %}`).
- `node-types.json` — generated node type metadata.
- `tree_sitter/` — vendored tree-sitter C runtime headers.
