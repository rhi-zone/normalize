Tree-sitter grammars maintained by this project. Each subdirectory is a standalone
tree-sitter grammar that can be compiled independently and published to crates.io.

Grammars here are written from scratch when the upstream or arborium grammar is absent,
incomplete, or uses a flat structure that blocks structured extraction (symbols, calls,
complexity). After local development and testing here, each grammar is published as its
own crate and referenced via `normalize-grammars`.

Contents:
- `jinja2/` — Jinja2 template grammar with named statement/expression node types
