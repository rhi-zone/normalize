# Polyglot Type Generator

**Status:** Draft / Exploration

## Problem

No single tool generates high-quality, idiomatic type definitions across multiple languages from common spec formats. Existing options:

| Tool | Coverage | Quality | Notes |
|------|----------|---------|-------|
| protoc | ~10 languages | Varies by plugin | Locked to protobuf |
| OpenAPI Generator | 50+ languages | Inconsistent | Some generators abandoned |
| quicktype | ~10 languages | Decent | Types only, limited customization |
| typeshare | 4 languages | Good | Rust-centric, simple types only |

**Gap:** A tool that is both wide (many languages) AND deep (idiomatic, high-quality output).

## Proposal

A standalone tool that:

1. **Reads** common spec formats (input)
2. **Produces** idiomatic type definitions (output)
3. **Prioritizes quality** over breadth initially

### Input Formats (Phase 1)

- JSON Schema (draft-07, 2020-12)
- OpenAPI 3.x (schema subset)
- Protocol Buffers (proto3)

### Input Formats (Later)

- AsyncAPI
- Smithy
- GraphQL SDL
- TypeSpec

### Output Languages (Phase 1)

Start narrow, go deep:

- **TypeScript** - most requested, good test case
- **Python** - dataclasses + type hints (3.10+)
- **Go** - structs + json tags
- **Rust** - serde derives

### Output Languages (Later)

- Kotlin (data classes)
- Swift (Codable)
- C# (records)
- Java (records or POJOs)

## Design Principles

### 1. Idiomatic Output

Each language backend should produce code that looks hand-written by an expert in that language:

```typescript
// Good: TypeScript idioms
export interface User {
  readonly id: string;
  name: string;
  email?: string;
  createdAt: Date;
}

// Bad: Generic/mechanical
export interface User {
  id: string | null;
  name: string | null;
  email: string | null;
  created_at: string | null;
}
```

### 2. Customizable Conventions

Different projects have different conventions:

```toml
[typescript]
naming = "camelCase"        # or "snake_case"
optional_style = "question" # or "union_undefined"
date_type = "Date"          # or "string" or "dayjs"
readonly = true

[python]
style = "dataclass"         # or "pydantic" or "typeddict"
naming = "snake_case"
optional_style = "Optional" # or "union_none"
```

### 3. Escape Hatches

When codegen isn't enough:

```yaml
# In spec or config
x-custom:
  typescript:
    import: "import { CustomType } from './custom'"
    type: "CustomType"
```

### 4. Deterministic Output

Same input + config = same output. Always. No timestamps, random orderings, or environment-dependent output.

### 5. Incremental / Watch Mode

For development workflows:

```bash
polytypes generate --watch specs/ --out generated/
```

## Architecture: `moss-codegen` Crate

Fits naturally in the moss monorepo as a new crate.

### Existing Infrastructure to Leverage

| Crate | Provides |
|-------|----------|
| `moss-jsonschema` | JSON Schema parsing |
| `moss-openapi` | OpenAPI parsing |
| `moss-languages` | Per-language modules (98 languages) |
| `moss-grammars` | Tree-sitter grammars for formatting |

### New Crate: `moss-codegen`

```
moss-codegen/
├── src/
│   ├── lib.rs
│   ├── input/           # Input format adapters
│   │   ├── jsonschema.rs
│   │   ├── openapi.rs
│   │   └── protobuf.rs
│   ├── ir.rs            # Intermediate representation
│   ├── output/          # Language backends
│   │   ├── typescript.rs
│   │   ├── python.rs
│   │   ├── go.rs
│   │   └── rust.rs
│   └── config.rs        # Per-language options
```

### CLI Integration

```bash
moss codegen \
  --input api.json \
  --format openapi \
  --lang typescript \
  --lang python \
  --out ./generated
```

### IR (Intermediate Representation)

Input formats normalize to a common IR before language backends:

```rust
enum Type {
    String,
    Integer { bits: u8, signed: bool },
    Float { bits: u8 },
    Boolean,
    Array(Box<Type>),
    Map { key: Box<Type>, value: Box<Type> },
    Optional(Box<Type>),
    Struct(StructDef),
    Enum(EnumDef),
    Ref(String),  // Reference to another type
}

struct StructDef {
    name: String,
    fields: Vec<Field>,
    docs: Option<String>,
}

struct Field {
    name: String,
    ty: Type,
    required: bool,
    docs: Option<String>,
}
```

### Relationship to `moss-languages`

`moss-languages` = parsing (extracting symbols from existing code)
`moss-codegen` = generation (producing new code from specs)

Could share:
- Language metadata (extensions, naming conventions)
- Formatter invocation (run prettier/black/gofmt on output)

## Quality Checklist (per language)

Before a language backend is "ready":

- [ ] Handles all JSON Schema types (string, number, boolean, array, object, null)
- [ ] Handles enums (string, numeric)
- [ ] Handles optional/nullable correctly
- [ ] Handles nested types
- [ ] Handles recursive types
- [ ] Handles allOf/oneOf/anyOf (where language supports)
- [ ] Naming conventions configurable
- [ ] Output compiles/type-checks
- [ ] Output is formatted (prettier, gofmt, rustfmt, black)
- [ ] Has integration tests against real-world schemas

## Open Questions

1. ~~**Where does this live?**~~ → `moss-codegen` crate in moss monorepo

2. **Validation codegen?** Just types, or also runtime validators?
   - Types only = simpler, broader use
   - With validators = more useful, more complex

3. **Client codegen?** Just types, or also API clients?
   - Types only = simpler, composable with any HTTP client
   - Full clients = more useful, much more complex

4. **Relationship to trellis?** Trellis outputs specs, moss-codegen consumes them. Clean boundary:
   ```
   trellis (Rust impl → specs) → moss codegen (specs → polyglot types)
   ```
   Separate repos, complementary tools.

5. **Name?**
   - `polytypes` - clear but generic
   - `moss codegen` - if it lives in moss
   - `arbor` - tree theme (types branch out)
   - `spore` - already taken in Rhizome
   - Something else?

## Non-Goals (for now)

- Full API client generation (just types)
- Runtime validation (just static types)
- Every language (start with 4, do them well)
- Every spec format (start with 3)
- Backward compatibility with OpenAPI Generator templates

## Success Criteria

1. TypeScript output passes `tsc --strict`
2. Python output passes `mypy --strict`
3. Go output passes `go vet`
4. Rust output passes `cargo clippy`
5. Real users prefer it over alternatives for supported languages
