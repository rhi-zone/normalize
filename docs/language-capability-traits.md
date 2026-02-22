# Language Capability Traits

Design doc for decomposing the monolithic `Language` trait into capability-based sub-traits.

## Precedent: LocalDeps

This split has already happened once. `LocalDeps` is a separate trait from `Language` precisely because only ~10/98 languages implement it meaningfully (filesystem/package discovery). The pattern is proven — the question is whether to apply it within `Language` itself.

## Problem

The current `Language` trait has 20+ required methods. Two growth axes are coupled when they shouldn't be:

- **Adding a language** forces implementing every method, even inapplicable ones (TOML has no complexity nodes, GLSL has no imports, config languages have no test symbols). `has_symbols()` exists precisely as a workaround for this — a smell.
- **Adding a feature** forces a sweep of all 98 existing language impls before the feature ships, or it doesn't ship at all.

In two years, with more features, both problems compound. The trait becomes a bottleneck.

## Proposed Design

Split `Language` into a required core plus optional capability traits:

```rust
// Required for all languages
trait LanguageCore: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn grammar_name(&self) -> &'static str;
}

// Optional capabilities — implement what your language supports
trait LanguageSymbols: LanguageCore {
    fn function_kinds(&self) -> &'static [&'static str];
    fn container_kinds(&self) -> &'static [&'static str];
    fn type_kinds(&self) -> &'static [&'static str];
    fn extract_function(&self, node: &Node, content: &str, in_container: bool) -> Option<Symbol>;
    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol>;
    fn extract_type(&self, node: &Node, content: &str) -> Option<Symbol>;
    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String>;
    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String>;
    fn is_test_symbol(&self, symbol: &Symbol) -> bool;
    fn visibility_mechanism(&self) -> VisibilityMechanism;
    fn is_public(&self, node: &Node, content: &str) -> bool;
    fn get_visibility(&self, node: &Node, content: &str) -> Visibility;
    fn public_symbol_kinds(&self) -> &'static [&'static str];
    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export>;
}

trait LanguageImports: LanguageCore {
    fn import_kinds(&self) -> &'static [&'static str];
    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import>;
    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String;
}

trait LanguageComplexity: LanguageCore {
    fn complexity_nodes(&self) -> &'static [&'static str];
    fn nesting_nodes(&self) -> &'static [&'static str];
    fn scope_creating_kinds(&self) -> &'static [&'static str];
    fn control_flow_kinds(&self) -> &'static [&'static str];
}

trait LanguageEdit: LanguageCore {
    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>>;
    fn body_has_docstring(&self, body: &Node, content: &str) -> bool;
    fn signature_suffix(&self) -> &'static str;
}

trait LanguageDebug: LanguageCore {
    fn debug_identifiers(&self) -> &'static [DebugIdentifier];
}

// Future capabilities slot in here without touching existing impls
```

Adding a language: implement `LanguageCore` + whatever capabilities apply. Ship it.

Adding a feature: add a new optional trait. Implement for the languages that matter. Coverage grows over time without blocking the feature.

## Dispatch Design

The hard part. Several options:

### Option A: Capability registry per language entry

```rust
struct LanguageEntry {
    core: Box<dyn LanguageCore>,
    symbols: Option<Box<dyn LanguageSymbols>>,
    imports: Option<Box<dyn LanguageImports>>,
    complexity: Option<Box<dyn LanguageComplexity>>,
    edit: Option<Box<dyn LanguageEdit>>,
}
```

Call sites use `entry.symbols.as_ref().map(|s| s.extract_function(...))`. Explicit, no magic, ugly at call sites.

### Option B: Capability query on a unified trait object

```rust
trait Language: LanguageCore {
    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> { None }
    fn as_imports(&self) -> Option<&dyn LanguageImports> { None }
    fn as_complexity(&self) -> Option<&dyn LanguageComplexity> { None }
    fn as_edit(&self) -> Option<&dyn LanguageEdit> { None }
}
```

Languages that support a capability override `as_symbols()` to return `Some(self)`. Single `dyn Language` everywhere, capability-query at use sites. Less allocation, cleaner registry, more ergonomic.

**Likely the right choice.** `dyn Language` remains the single dispatch type.

### Option C: Enum-based capabilities

Skip traits entirely for the optional parts; use an enum of capability sets. More explicit but less extensible.

## Tradeoffs

| | Monolithic | Capability traits |
|---|---|---|
| Adding a language | All-or-nothing, high bar | Incremental, low bar |
| Adding a feature | 98-language sweep or nothing | Ship for N languages, grow coverage |
| Silent gaps | Impossible (forced impl) | Explicit via `Option` return |
| Dispatch | Simple `dyn Language` | `lang.as_symbols()?` at call sites |
| Discovery | One trait = obvious contract | Need docs/registry to know what X supports |
| `has_symbols()` smell | Present | Gone — it's just `lang.as_symbols().is_some()` |

The "silent gaps" tradeoff is real but manageable: call sites that need symbols get `Option<&dyn LanguageSymbols>` and handle the None case explicitly. This is better than the current situation where TOML returns empty vecs from symbol methods and callers can't tell "no symbols" from "not implemented".

## When to Split

The trigger is sparsity: if a significant fraction of languages don't implement a capability (return empty/stub), it should be an optional trait rather than a required method.

- **>80% of languages implement it** → stays in core, required
- **<50% of languages implement it** → separate optional trait

`embedded_content` is already past this threshold — only Vue, HTML, and a handful of others implement it meaningfully. `has_symbols()` exists as a workaround for the same problem with symbols in config languages.

The rule of thumb: **if you'd write `has_X()` to guard a method, X should be an optional trait instead.**

Don't split preemptively. Wait until either: (a) the sparsity is visible at implementation time — you're writing stubs for more than half the languages — or (b) a second `has_X()` workaround accumulates.

## Migration

Not a flag-day migration. The current `Language` trait can stay while we design Option B's `as_*` query methods. Steps:

1. Add `as_symbols()`, `as_imports()`, etc. to the current `Language` trait with `None` defaults
2. For languages that fully implement symbols, override `as_symbols()` to return `Some(self)`
3. Call sites migrate to use capability queries where they need to distinguish "not supported" from "empty"
4. Once all call sites migrate, the flat methods on `Language` can be moved into the sub-traits
5. Remove `has_symbols()` — it's now derivable

This is incremental and doesn't break the 98 existing impls on day one.

## What Doesn't Belong on Language

Lint opinions are not language properties. A language doesn't inherently know that `console.log` is bad — that's a convention enforced by rules. The `Language` trait describes structure (grammar, AST shape, visibility, imports); the rules system owns opinions about what's acceptable.

This came up when considering `debug_identifiers()` as a trait method. The right home for that knowledge is `.scm` rule files, not the `Language` trait — rules are the unit of lint implementation, tags are the unit of user intent.

## Open Questions

1. **Granularity**: Is `LanguageSymbols` the right split, or should visibility be its own capability? Err toward fewer, coarser capabilities to start.
2. **`node_name()`**: Helper used internally — keep on `LanguageCore` or move to a non-trait utility function?
3. **`embedded_content()`**: Very few languages need this (Vue, HTML). Good candidate for a thin `LanguageEmbedded` trait.
4. **Compile-time capability checking**: Can we use type-level capabilities for the static dispatch path (non-dyn usage)? Probably not worth the complexity.
