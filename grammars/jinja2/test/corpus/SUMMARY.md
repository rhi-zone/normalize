Corpus test files for the Jinja2 tree-sitter grammar. Each `.txt` file contains named test cases in tree-sitter corpus format (input + expected CST). Two files:
- `expressions.txt` — tests covering the full expression language (binary/unary operators, filters, tests, calls, attribute/subscript access, literals).
- `statements.txt` — tests covering all statement types (extends, import, from, include, block, macro, for, if, call, filter_block, set, set_block, with, autoescape, trans, do, debug, raw).

Run with `tree-sitter test` from `grammars/jinja2/`.
