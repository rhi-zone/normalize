# normalize-semantic-facts — Overview

## What it does

Extracts **semantic facts** from code, lowers them into a common intermediate
representation (the fact IR), and identifies when the same fact appears in multiple
places across a codebase — potentially across different languages and syntactic forms.

This is a different axis from `normalize-facts`, which extracts *syntactic* facts
(symbols, imports, calls — "what exists and how it's wired"). `normalize-semantic-facts`
asks "what does this code *mean*", and whether that meaning has already been stated
somewhere else.

## Core concepts

### Semantic fact

A unit of meaning, not syntax. "Entity `Lesson` has field `status` of type
`enum(scheduled, in_progress, completed, cancelled)`" is one semantic fact, regardless of
whether it's expressed as a SQL column definition, a TypeScript type field, a validation
schema enum, or an API parameter. Four syntactic forms, one fact.

### Fact IR

The intermediate representation all facts lower to. Language-specific extractors compile
CST nodes into fact IR nodes. Two source-level constructs that produce the same IR *are*
the same fact — identity is equality on the IR, nothing fuzzier.

Designing the IR so that genuinely equivalent things converge — without collapsing
things that are actually different — is the core design challenge of this crate. See
[Open questions](#open-questions).

### Extractors

Language-specific lowering passes. Tree-sitter gives the CST; an extractor walks it and
emits fact IR nodes for the structural declarations it recognizes:

- Type/entity declarations (name, fields, field types)
- Enum definitions (name, variants)
- Function/method signatures (name, parameters, parameter types, return type)
- Field constraints (nullable, max length, etc.)
- Relationships/references between entities

### Three-state classification

Not every piece of code can be lowered to a fact IR node. The classifier has three
states:

- **Extracted** — successfully lowered to fact IR (structural declarations).
- **Uncertain** — recognized as potentially meaningful but not reliably lowerable
  (behavioral logic, complex expressions). Acknowledged, not normalized.
- **Unclassified** — not attempted (whitespace, comments, imports, boilerplate syntax).

The three-state split matters because it keeps the tool honest: "not extracted" and "not
meaningful" are different claims, and collapsing them would either overclaim coverage or
hide real restatement in the uncertain bucket.

## Pipeline

```
Source code
  → tree-sitter CST (per-language grammar)
  → language-specific extractor (per-language)
  → fact IR nodes (language-agnostic)
  → identity/deduplication (IR equality)
  → per-fact restatement report
```

## Output

Per fact: the normalized fact, its restatement count, and the list of locations
(file:line) where it appears. Sorted by restatement count — most-restated first, i.e.
biggest compression targets first.

```
entity(Lesson).field(status): enum(scheduled, in_progress, completed, cancelled)
  restatements: 8
  locations:
    migrations/001.sql:12          (SQL column definition)
    src/types.ts:34                (TypeScript type field)
    src/schema.generated.ts:7      (validation schema)
    src/sqliteRepo.ts:991          (query result parsing)
    ...
```

## What's tractable vs. hard

**Tractable — structural facts.** Type declarations, entity definitions, enum values,
function signatures, field constraints. These are visible in the AST and lower cleanly
to IR.

**Hard — behavioral facts.** Business logic constraints ("cancel checks status isn't
already cancelled"), control flow decisions, algorithmic logic. These land in the
uncertain bucket: the tool acknowledges they exist but does not attempt to normalize
them.

**Hard — cross-language identity.** `status TEXT NOT NULL` in SQL and
`status: LessonStatus` in TypeScript are the same fact, but seeing that requires
resolving `LessonStatus` to the actual enum it aliases. That's cross-file analysis at
minimum, and possibly cross-language type resolution. Nothing here claims this is solved
— see below.

## Open questions

- What does the fact IR actually look like — what are its node types, and where's the
  line between "structure" and "constraint" in the schema?
- How deep does type resolution go? Direct aliases only? Generics? Computed/derived
  types? Each step out is a step further from tractable.
- Should the IR capture constraints (nullable, max length) as part of a fact's identity,
  or treat them as separate facts attached to the same entity/field? Affects what counts
  as a "restatement" versus a "partial restatement."
- Storage: extend the existing `normalize-facts` SQLite index with new tables, or keep
  this a separate store? Bears on whether restatement queries can join against the
  existing symbol/import graph.
- How does this relate to `normalize-code-similarity`? That crate finds *syntactically*
  similar code (MinHash LSH, normalized AST hashing); this crate is trying to find
  *semantically* identical facts across syntactically unrelated forms. Different
  problem, possibly complementary — worth revisiting once the IR is real.

None of these are decided. This document describes the shape of the problem and the
pipeline it implies, not a finished design.

## Motivation

A production SaaS with 626k lines was found to contain roughly 10-15k lines of actual
business logic. The rest is ceremony — the same structural facts restated across
persistence, types, validation, API, and UI layers. This crate exists to measure that
redundancy precisely, per fact, so a codebase's entropy can be seen as real (behavioral)
versus restated (structural) rather than eyeballed from LoC counts.
