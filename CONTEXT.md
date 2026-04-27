# Ubiquitous Language

Domain vocabulary for normalize. Use these terms precisely in code, docs, and conversations.

## Symbol
_Avoid:_ function, definition, declaration

A named definition extracted from source code via tree-sitter queries: function, class, type, variable, etc. Stored as a `Symbol` struct with kind, visibility, and optional children. The fundamental unit of the structural index.

## Export
_Avoid:_ public symbol, re-export (different concept)

A symbol explicitly declared as public via language visibility mechanisms (modifiers, naming conventions, package-level scoping). Distinct from ReExport (a pass-through) and from "export" in the sense of outputting data.

## Import
_Avoid:_ dependency, require, use

A reference to code from another file or module, with a resolution status: local (resolved to a known path), remote (external package), or ambient (global/built-in). Not all imports are resolvable.

## ReExport
_Avoid:_ export, re-export alias

An import that is re-exposed publicly by a module — a pass-through dependency. Primarily a JavaScript/TypeScript concept. Distinct from a plain Export (defined here) and a plain Import (consumed here).

## FileIndex
_Avoid:_ index, cache, database

The central facts database: symbol/import/call graph stored in SQLite at `.normalize/index.sqlite`. Rebuilt via `normalize structure rebuild`. Separate from the CA Cache.

## CA Cache
_Avoid:_ index, cache (ambiguous)

A separate content-addressed SQLite store for extraction payloads, keyed by file content hash. Enables incremental rebuilds: if the file hash hasn't changed, extraction is skipped and the CA Cache entry is reused.

## Language (trait)
_Avoid:_ language (the human language)

The core Rust trait decomposed into capability sub-traits: `LanguageCore`, `LanguageSymbols`, `LanguageImports`, `LanguageComplexity`, `LanguageEdit`. A Language implementation encapsulates all knowledge of how to extract facts from a specific language's source. Distinguish from "the language" (e.g. TypeScript-the-language vs TypeScript-the-Language-impl).

## GrammarLoader
_Avoid:_ parser, grammar

The component that dynamically loads tree-sitter grammar `.so` files at runtime. Grammar load failures are loud — files with unavailable grammars are skipped with a warning, never indexed as empty.

## Container
_Avoid:_ module, namespace (as interchangeable)

A behavioral property of certain `SymbolKind` variants (Module, Class, Namespace) that can contain other symbols. Not a distinct type — it describes which symbol kinds have children.

## Extraction
_Avoid:_ parsing, analysis, indexing

The process of running tree-sitter on source and converting CST nodes into typed domain objects (Symbols, Imports, calls). Extraction is one phase of `structure rebuild`; the result is stored in the CA Cache and then merged into the FileIndex.

## PlannedEdit
_Avoid:_ edit, patch, change, refactor

A structured edit operation: file path, location, and replacement text. Supports dry-run and shadow application. The typed representation of a code modification before it touches the filesystem.

## Rule (four kinds)
_Avoid:_ using "rule" without qualification

Four distinct rule types coexist in normalize — always specify which:
- **Syntax rule** — tree-sitter query matching source patterns; evaluated by `normalize-syntax-rules`
- **BuiltinRule** — a syntax or Datalog rule embedded at compile time as text; "builtin" means shipped with normalize, not "written in Rust"
- **FactsRule** — a Datalog rule evaluated by the Ascent-based engine; operates over the FileIndex
- **NativeRuleDescriptor** — a rule implemented in Rust via the external-process model; previously called "builtin" in some contexts (the old dylib model was removed)

## Fact
_Avoid:_ finding, result, diagnostic

A Datalog relation in the FactsRule engine — a row in a relation table (e.g. "symbol X is exported from file Y"). Facts are the input and output of Datalog evaluation. Distinct from Findings.

## Finding
_Avoid:_ fact, diagnostic, error, result

A diagnostic produced by a rule evaluation: a location, a message, and a severity. Findings come from syntax rules, facts rules, and native rules. Stored in the findings cache (`.normalize/findings-cache.sqlite`).

## Severity
_Avoid:_ level, priority

Four levels: Error, Warning, Info, Hint. `"note"` is an alias for Info. Severity determines CI gate behavior — only errors gate by default. `"warn"` maps to Warning.

## Fan-in / Fan-out
_Avoid:_ dependencies, dependents

**Fan-in** (afferent coupling): the number of modules that depend on X. **Fan-out** (efferent coupling): the number of modules X depends on. The terms are directional — always specify which direction.

## Instability
_Avoid:_ coupling, fragility

A derived metric: `fan-out / (fan-in + fan-out)`. 0 = maximally stable (many dependents, few dependencies). 1 = maximally unstable (few dependents, many dependencies). High instability means changes to this module ripple outward less but the module itself is more likely to change.

## Blast Radius
_Avoid:_ dependents, impact, fan-in

The set of modules that would be affected by a change to a given module — a transitive fan-in calculation. Not just direct dependents.

## API-first
_Avoid:_ CLI-first, command-first

The design principle that service methods return typed data structures; the CLI renders them. CLI aesthetics (flag names, output format) must never drive the shape of the underlying data. The same data should be consumable as JSON, JSONL, or pretty-printed without changing the service.

## `view` vs `list`
_Avoid:_ using interchangeably

`view` is for a single identified item (a specific symbol, file, or entity). `list` is for a collection (all symbols in a file, all files in a project). The distinction is load-bearing in CLI design — conflating them produces commands that don't compose.
