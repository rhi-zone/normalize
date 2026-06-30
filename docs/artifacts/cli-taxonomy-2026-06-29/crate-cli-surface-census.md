# Crate CLI Surface Census — 2026-06-29

Investigation of CLI surface across every workspace crate. Resolves a factual disagreement: a prior
investigation claimed "exactly 8 crates define a #[cli] service" and that five compute crates
(normalize-analyze, normalize-architecture, normalize-metrics, normalize-code-similarity, normalize-graph)
are "pure libraries with zero CLI."

## Methodology

Search criteria applied to each crate:
1. Does `Cargo.toml` declare a `cli` feature (`cli =` in `[features]`)?
2. Does it depend on `server-less` or declare it as optional?
3. Does source contain `#[cli` attribute (server-less proc macro)?
4. Does source implement `OutputFormatter` (format_text / format_pretty)?
5. Is there a standalone binary (`[[bin]]`)?
6. Is it mounted into the normalize binary (with `features = ["cli"]`)?

Commands run:
```
grep -r "#\[cli" crates/ --include="*.rs" -l
grep -r "OutputFormatter" crates/ --include="*.rs" -l
grep -rl 'server-less|server_less' crates/ --include="*.toml"
grep -l 'cli\s*=' crates/*/Cargo.toml
```

---

## Workspace Crate Count

**47 workspace members** total (from `Cargo.toml [workspace.members]`):
- 45 named crates (includes normalize-grammars publish=false and xtask publish=false)
- 1 benches workspace member
- 1 xtask workspace member

CLAUDE.md states "38 crates + 2 publish=false" (40 total). The workspace has grown to 47 members —
approximately 7 crates added since CLAUDE.md was last updated.

---

## Classification

### Class A — Full #[cli] service, mounted into the normalize binary

These crates have `#[cli]`-decorated services and are activated in the normalize binary with
`features = ["cli"]`:

| Crate | Mounted as | Activation in normalize/Cargo.toml |
|---|---|---|
| normalize (main binary) | root service | N/A — is the binary |
| normalize-budget | `budget` field on NormalizeService | `features = ["cli"]` |
| normalize-cfg | `cfg` field on NormalizeService | `features = ["cli"]` (also `default = ["cli"]`) |
| normalize-knowledge-graph | `kg` field on NormalizeService | `features = ["cli"]` |
| normalize-ratchet | `ratchet` field on NormalizeService | `features = ["cli"]` |
| normalize-rules | `rules` field on NormalizeService | `features = ["cli"]` |

**Total: 6 crates** (5 external + 1 main binary).

Evidence — from `crates/normalize/src/service/mod.rs`:
```rust
budget: normalize_budget::service::BudgetService,
cfg: normalize_cfg::service::CfgService,
kg: normalize_knowledge_graph::service::KgCliService,
ratchet: normalize_ratchet::service::RatchetService,
rules: normalize_rules::RulesService,
```

### Class A-standalone — Full #[cli] service, standalone binary (NOT mounted into normalize)

These crates have a `[[bin]]` with `required-features = ["cli"]` and a proper `#[cli]` service,
but the normalize binary does NOT activate their `cli` feature:

| Crate | Binary name | Status in normalize binary |
|---|---|---|
| normalize-filter | `normalize-filter` | Used with `features = ["config"]` only — CLI NOT activated |
| normalize-facts | (unnamed standalone) | Used without any features — CLI NOT activated |
| normalize-syntax-rules | `normalize-syntax-rules` | Used without features (only default `["fix"]`) — CLI NOT activated |

**Total: 3 crates** with standalone CLI binaries not mounted into the main normalize binary.

Evidence:
- normalize-filter/Cargo.toml: `[[bin]] name = "normalize-filter" required-features = ["cli"]`
- normalize-facts/Cargo.toml: `[[bin]] required-features = ["cli"]`; lib.rs: `#[cfg(feature = "cli")] pub mod service;`
- normalize-syntax-rules/Cargo.toml: `[[bin]] name = "normalize-syntax-rules" required-features = ["cli"]`
- normalize/Cargo.toml: `normalize-filter = { ..., features = ["config"] }` (no `cli`)
- normalize/Cargo.toml: `normalize-facts = { ... }` (no features — defaults to `default = []`)
- normalize/Cargo.toml: `normalize-syntax-rules = { ... }` (no features — defaults to `default = ["fix"]`)

### Class B — Partial CLI surface (OutputFormatter impls and/or cli feature, but no complete mounted #[cli] service)

| Crate | cli feature | OutputFormatter | #[cli] service | Notes |
|---|---|---|---|---|
| normalize-context | YES (`cli = ["dep:normalize-output"]`) | YES (gated by `#[cfg(feature = "cli")]`) | NO | OutputFormatter impls for ContextListReport and ContextReport; the `#[cli]` service is in normalize's own `service/context.rs`; normalize uses with `features = ["cli"]` |
| normalize-graph | NO | YES (unconditional — no cfg gate) | NO | Implements OutputFormatter for DependentsReport and GraphReport in lib.rs; depends on normalize-output unconditionally; no cli feature exists |
| normalize-native-rules | NO | YES (unconditional) | NO | Implements OutputFormatter for BudgetRulesReport, StaleDocsReport, CheckRefsReport, CheckExamplesReport, MissingSummaryReport, StaleSummaryReport, RatchetRulesReport |
| normalize-semantic | YES (`cli = ["dep:server-less", "dep:schemars"]`) | YES (unconditional impl, schemars gated) | NO | server-less listed as optional dep but `#[cli]` macro is never applied; NOT a dependency of the normalize binary at all; the service comment says "CLI method lives in normalize/src/service/facts.rs" but that wiring is absent |
| normalize-session-analysis | NO | YES (unconditional) | NO | Implements OutputFormatter for SessionAnalysisReport |

**Total: 5 crates** with partial CLI surface.

### Class C — Pure library (no cli feature, no CLI code)

All remaining 33 named crates (verified no `cli =` in Cargo.toml, no OutputFormatter impls, no `#[cli]` attributes):

normalize-analyze, normalize-architecture, normalize-metrics, normalize-code-similarity,
normalize-manifest, normalize-cli-parser, normalize-surface-syntax, normalize-typegen,
normalize-core, normalize-derive, normalize-facts-core, normalize-facts-rules-api,
normalize-grammars, normalize-languages, normalize-language-meta, normalize-openapi,
normalize-ecosystems, normalize-package-index, normalize-chat-sessions, normalize-tools,
normalize-local-deps, normalize-output (defines the trait itself), normalize-path-resolve,
normalize-shadow, normalize-edit, normalize-deps, normalize-scope, normalize-rules-config,
normalize-refactor, normalize-module-resolve, xtask, benches

**Total: 33 crates** (including xtask and benches).

---

## Tally

| Class | Count |
|---|---|
| A (mounted #[cli] service) | 6 (1 main + 5 external) |
| A-standalone (unmounted #[cli] binary) | 3 |
| B (partial CLI surface) | 5 |
| C (pure library) | 33 |
| **Total** | **47** |

---

## Verdict on the 5 Compute Crates

### normalize-analyze
**Class C — pure library. The prior "zero CLI" claim is CORRECT.**

`Cargo.toml [features]`: absent (no features section at all).
```toml
[dependencies]
serde = { workspace = true }
schemars = "1"
```

Source: `ranked.rs` line 8 mentions OutputFormatter only in a doc comment (`//! OutputFormatter::format_text()`). No OutputFormatter impl, no server-less dep, no `#[cli]`.

### normalize-architecture
**Class C — pure library. The prior "zero CLI" claim is CORRECT.**

`Cargo.toml [features]`: absent.
```toml
[dependencies]
normalize-facts = { ... }
normalize-graph = { ... }
normalize-languages = { ... }
libsql.workspace = true
serde.workspace = true
schemars = "1"
```

Source: `lib.rs` line 4 says `//! Report structs and OutputFormatter impls live in the normalize crate.` — only a doc comment. No OutputFormatter impl in this crate.

### normalize-metrics
**Class C — pure library. The prior "zero CLI" claim is CORRECT.**

`Cargo.toml [features]`: absent.
```toml
[dependencies]
serde = { workspace = true }
schemars = "1"
anyhow = "1"
```

Source: no OutputFormatter, no server-less, no `#[cli]`.

### normalize-code-similarity
**Class C — pure library. The prior "zero CLI" claim is CORRECT.**

`Cargo.toml [features]`: absent.
```toml
[dependencies]
tree-sitter = "0.26"
streaming-iterator = "0.1"
normalize-languages = { ... }
```

Source: no OutputFormatter, no server-less, no `#[cli]`.

### normalize-graph
**Class B — partial CLI surface. The prior "zero CLI" claim is INACCURATE.**

`Cargo.toml [features]`: absent — no `cli` feature exists.
```toml
[dependencies]
serde = { workspace = true }
schemars = "1"
normalize-output = { path = "../normalize-output", version = "0.3.2" }
nu-ansi-term = "0.50"
```

Source (`lib.rs`), with no cfg-gate:
```
10:  use normalize_output::OutputFormatter;
188: impl normalize_output::OutputFormatter for DependentsReport {
189:     fn format_text(&self) -> String {
196:     fn format_pretty(&self) -> String {
1082: impl OutputFormatter for GraphReport {
1083:     fn format_text(&self) -> String {
1238:     fn format_pretty(&self) -> String {
```

`normalize-graph` unconditionally depends on `normalize-output` and implements `OutputFormatter` for
two report types (`DependentsReport`, `GraphReport`) without any feature gate. These impls are
always compiled in for any consumer. This is CLI surface code embedded in a "compute" crate — it
violates the CLAUDE.md principle that "Generally useful functionality belongs in its own crate" and
that CLI wiring should be separate from domain logic. No `#[cli]` service, but OutputFormatter
impls mean it is not a pure library.

---

## Discrepancies with Prior "8 Crates, 5 Pure Libraries" Claim

### "#[cli] service count"
The prior claim of "exactly 8 crates" is ambiguous but roughly consistent:

If you count external crates with `#[cli]` services (excluding the main normalize binary):
- normalize-budget, normalize-cfg, normalize-facts, normalize-filter, normalize-knowledge-graph,
  normalize-ratchet, normalize-rules, normalize-syntax-rules = **8 crates**

This count is correct IF you mean "any crate with a `#[cli]` service in its source" regardless of
whether that service is mounted. However, only 5 of those 8 are actually mounted into the normalize
binary (the 5 listed as Class A above). The other 3 (filter, facts, syntax-rules) are standalone
binaries.

### "5 pure libraries"
- normalize-analyze: CONFIRMED pure library ✓
- normalize-architecture: CONFIRMED pure library ✓
- normalize-metrics: CONFIRMED pure library ✓
- normalize-code-similarity: CONFIRMED pure library ✓
- normalize-graph: **INCORRECT** — has OutputFormatter impls unconditionally compiled; is Class B

The claim that normalize-graph has "zero CLI" is wrong. It has `normalize-output` as an unconditional
dependency and implements `OutputFormatter` for `DependentsReport` and `GraphReport` without any
feature gate. These impls are active whenever the crate is compiled.

---

## Additional Finding: normalize-semantic Orphaned

`normalize-semantic` declares `cli = ["dep:server-less", "dep:schemars"]` and has OutputFormatter
impls, but:
1. It is NOT a dependency of the normalize binary at all.
2. It has no `#[cli]` service (the proc macro is never applied).
3. Its own `service.rs` says "The CLI method itself lives in `crates/normalize/src/service/facts.rs`"
   but that wiring is absent from the normalize binary.

This is a partially-built integration that was never connected.
