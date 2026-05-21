# Graph-Substrate: 3-Primitive CLI Design

The knowledge graph CLI exposes three indivisible operations.

## Primitives

```
kg read  [SELECTOR]              # 0..n units out
kg write [SELECTOR] [TRANSFORM]  # mutate via jq; null deletes; no selector → create from stdin
kg walk  <ID> <JQ-OVER-LINKS>   # graph traversal
```

### Why exactly three

**put/patch were the same write.** `create` and `set` differed only by whether a jq expression was a constant or a path update. They map to the same `write` operation — the expression shape controls behavior, not the verb.

**get/query were the same read.** `get` was read-by-id, `query` was read-by-predicate. Same operation, different selector type. Both collapse to `read` with an optional `-q` jq predicate.

**rm was write-null.** `delete` was `write a 'null'`. Not a primitive.

**walk earns its own verb.** The distinguishing property: each hop's selector depends on the previous unit's output. That's a fold over the graph — irreducible to `read` (which maps, not folds). `neighbors` was a special case of walk with a fixed link expression.

### Selector grammar

- Bare positional string → id (O(1) file lookup).
- `-q '<jq-predicate>'` → scan all units, filter where predicate is truthy.
- Absent (read only) → all units.

### Transform grammar (write only)

A jq expression applied to the matched unit's full JSON:
- Returns a unit-shaped object → stored as the new unit.
- Returns `null` → unit is deleted.
- Returns anything else → error.

When no selector is given, stdin is read as the new unit (`.id` is the key).

### Walk

`kg walk <id> '<jq-expr>'` emits units reachable by repeatedly applying the jq expression to extract link target IDs from each unit. Traversal is BFS, de-duped by id. `--depth N` limits hops (0 = unlimited). `--include-start` includes the starting unit in output (default off).

## Indivisibility discipline

A unit is one thing that cannot be meaningfully split non-degenerately. This constraint drives the design: operations on units are either reads (no mutation, any selector) or writes (mutation via jq, any selector), plus the one traversal operation that cannot reduce to either.

Adding operations (link/unlink, edges listing) violated this: they were special cases of write with a specific jq path, or read with a post-filter. Naming them separately created false surface area — learnable names for things that were already expressible.

## Storage

Units live at `<normalize_dir>/kg/<id>.md` with YAML frontmatter. Links are stored in each unit's `links` frontmatter field — no shared mutable log. This gives per-unit ownership: branches that add edges to different units merge cleanly.

The `links` field shape: `[{kind: "references", to: "target-id", metadata?: {...}}]`.

The jq walk expression for following links: `.metadata.links[].to`.
