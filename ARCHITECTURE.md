# ARCHITECTURE.md

Reference for agents extending normalize. Read this before adding a crate,
a service method, a language, an ecosystem, or a rule.

This is the "how it fits together" doc. For design tenets see
`docs/philosophy.md`; for the per-command CLI surface see `docs/cli-design.md`;
for the legacy high-level sketch see `Architecture.md` (lowercase) — that file
is the elevator pitch, this one is the contract.

## 1. Architectural overview

### Workspace shape

One Cargo workspace, ~45 published crates plus `xtask` and `benches`. The
binary lives in `crates/normalize/`; everything else is library. The binary
crate is consumer-of-the-ecosystem, not home-of-reusable-logic.

Top-level layout:

```
crates/
  normalize/                      binary + service-layer wiring
  normalize-core/                 widely-shared primitives (re-exports)
  normalize-derive/               proc macros
  normalize-output/               OutputFormatter trait, diagnostics types

  normalize-languages/            Language trait + ~98 implementations
  normalize-language-meta/        capability metadata, orthogonal to syntax
  normalize-grammars/             tree-sitter grammar loading (publish=false)

  normalize-ecosystems/           Ecosystem trait + 12 package managers
  normalize-local-deps/           LocalDeps trait (installed-on-disk discovery)
  normalize-package-index/        PackageIndex trait (apt/brew/registry)

  normalize-facts/                fact extraction + SQLite index
  normalize-facts-core/           Symbol/Import/Export/TypeRef value types
  normalize-facts-rules-api/      Relations + Diagnostic types for fact rules
  normalize-facts-rules-interpret/ascent-interpreter Datalog engine

  normalize-rules/                rule orchestration (run/list/show/enable...)
  normalize-syntax-rules/         tree-sitter query rule engine
  normalize-native-rules/         native checks (check-refs, ratchet, ...)
  normalize-rules-config/         shared config types for rule engines

  normalize-knowledge-graph/      .normalize/kg/ unit store + 3-primitive CLI
  normalize-context/              .normalize/context/ frontmatter resolver
  normalize-tools/                external linter/formatter/test orchestration

  normalize-{view, edit, deps, filter, scope, graph, analyze, ...}/
                                  per-capability crates owning their commands
```

### Library-first / CLI-second discipline

normalize is an **API that happens to have a CLI**. Every command goes:

```
data extraction layer  →  typed Report struct  →  OutputFormatter / server-less
```

Report structs derive `Serialize + JsonSchema`. The CLI layer (`#[cli]`) is
generated; JSON, JSONL, jq filtering, and JSON Schema introspection come for
free. If you find yourself designing a CLI flag and the underlying data is
"whatever the print loop emits", stop — design the Report struct first.

Consequences for an extending agent:
- Returning `String` from a service method is almost always wrong. Return
  `Result<SomeReport, String>` where `SomeReport: Serialize + JsonSchema`.
- Implement `OutputFormatter::format_text(&self)` on the Report. `--json`
  and `--jq` come from `serde`; you do not write them.
- `format_pretty()` is opt-in for human-friendly output with color.

### Service layer pattern (`#[cli]`)

`crates/normalize/src/service/mod.rs` declares `NormalizeService` and
exposes top-level subcommands via `#[cli]`. Sub-services (analyze, view,
facts, rules, …) live in `service/<area>.rs` or in their own crate's
`service` module. Each annotated method becomes:

- a CLI subcommand (`normalize <verb> [flags]`)
- an MCP tool callable from `normalize serve mcp`
- an HTTP endpoint from `normalize serve http`
- a JSON Schema entry in the generated CLI snapshot

The macro lives in `server-less` (sibling repo at
`~/git/rhizone/server-less/`). When the macro misbehaves, fix server-less
— do not document a workaround in normalize.

Pattern:

```rust
#[server(group = "core")]
#[cli(display_with = "display_output")]
pub fn frob(
    &self,
    #[param(positional, help = "...")] target: String,
    #[param(short = 'r', help = "...")] root: Option<String>,
    pretty: bool,
    compact: bool,
) -> Result<FrobReport, String> { ... }
```

`display_output` bridges to `OutputFormatter` honouring the shared
`pretty`/`compact` cell on the parent service. Look at any existing service
method as a template — `service/view.rs` and `service/docs.rs` are clean
examples.

### Crate-per-capability extraction

A new top-level crate is justified only when **both** of these hold (per
`CLAUDE.md`):

1. It has multiple actual dependents in the workspace, **or** it is
   plausibly useful standalone (someone would publish-and-use it without
   normalize: e.g. `normalize-graph`, `normalize-code-similarity`).
2. The contents are domain logic — algorithms, data models, extraction —
   not CLI wiring for one command.

"Could theoretically be reused someday" does not count. CLI wiring (Report
structs, `OutputFormatter` impls, `#[cli]` service methods) for a feature
**lives in the crate that owns that feature**. The main `normalize` crate
only mounts sub-services; it owns no domain logic.

If neither condition is met, the code belongs in
`crates/normalize/src/commands/<name>.rs` or — better — inside the existing
crate that already owns the surrounding feature.

## 2. Crate boundary rules

### When to create a new crate

- The capability has a coherent public surface (trait + types + entrypoints)
  another workspace crate would consume.
- You can write a 1-paragraph description of it that does not contain the
  word "miscellaneous".
- It is publishable to crates.io as-is (no path deps — see Hard Constraints
  in CLAUDE.md).

### When NOT to create a new crate

- It is only used by one command in `normalize`. Put it in
  `crates/normalize/src/commands/`.
- It only exists to "tidy up" a long file. Refactor in-place.
- It is a thin re-export shim. Re-export from the existing owner instead.
- It would force a path dependency to break.

### When to extend an existing crate

If your new code is the same **kind of thing** as what is already in a
crate (another language → `normalize-languages`; another ecosystem →
`normalize-ecosystems`; another native rule → `normalize-native-rules`),
add it there with a feature flag if appropriate. Do not stand up a parallel
crate.

### When to split an existing crate

When the crate has two distinct consumer surfaces (library API and CLI;
rules engine and fixer; extraction and storage). Use feature flags first
(`cli`, `fix`); split into separate crates only when feature gating becomes
load-bearing for the dependency graph.

### Feature flags: capability surfaces, not optimisations

Feature flags exist to let downstream consumers opt out of capability
surfaces they don't want — not to micro-optimise dependency closure. Current
conventions:

- `cli` (default = true): server-less service registration, OutputFormatter
  glue, CLI-specific report wiring. Library-only consumers pass
  `default-features = false`.
- `fix` (default = true): `PlannedEdit` / autofix surfaces of a rules crate.
- `lang-*` / `langs-*`: per-language and language-group flags in
  `normalize-languages`.
- `cargo` / `npm` / `go` / `python` / …: per-ecosystem flags in
  `normalize-ecosystems`.

## 3. Per-area conventions

### 3.1 Languages — `normalize-languages`

**Abstraction:** the `Language` trait (`crates/normalize-languages/src/traits.rs`).
Each language is a zero-sized struct (`Python`, `Rust`, `TypeScript`, …)
implementing `Language`. Capability traits layer on top:

- `LanguageSymbols` (marker; opted into via `as_symbols()` returning `Some(self)`)
- `LanguageEmbedded` (for Vue, HTML, Svelte: extracts JS/CSS sub-blocks)
- `ModuleResolver` (returned from `Language::module_resolver()` for languages
  with a module system; `None` for shell-like languages)

**What the trait covers** (read `traits.rs` for the full surface):

- `name()`, `extensions()`, `grammar_name()` — identity.
- `extract_docstring`, `extract_attributes`, `extract_implements`,
  `build_signature`, `refine_kind` — symbol-building hooks called by the
  generic tree-sitter extractor.
- `extract_imports`, `format_import` — import lifting.
- `get_visibility`, `is_test_symbol`, `test_file_globs` — filtering.
- `container_body`, `body_has_docstring`, `analyze_container_body` — edit
  support (used by `normalize edit insert/append/prepend`).
- `extract_module_doc` — file-level summary (used by `view`).
- `module_resolver` — cross-file import resolution (Phase 0).

**Grammars** come from arborium (Amos Wenger's curated set) or — for
languages outside arborium — we write our own (Jinja2 set the precedent).
Never pull in random tree-sitter grammars from the ecosystem. Grammars are
shared libraries (`.so`/`.dylib`/`.dll`) loaded at runtime via `libloading`
through `normalize-grammars`. `cargo xtask build-grammars` builds them.

**Queries are first-class.** `*.tags.scm`, `*.imports.scm`, `*.calls.scm`,
`*.complexity.scm`, `*.types.scm`, `*.decorations.scm`, `*.cfg.scm` live
under `crates/normalize-languages/src/queries/`. The `GrammarLoader`
auto-loads them. **Node classification belongs in `.scm`; extraction
(getting names/fields from identified nodes) belongs in Rust.** This rule
extends to runner-level filters in other crates — if you find yourself
writing `if grammar_name == "rust" { ... }` or a `RUST_FOO_QUERY: &str`
constant in `normalize-syntax-rules` or any other language-agnostic crate,
stop. The query goes in `queries/<lang>.<purpose>.scm` and is loaded
through `GrammarLoader` like the others.

**To add a new language** — see Extension Recipes §9.1.

**Where the line falls:** `normalize-languages` is anything tree-sitter,
syntax-derived, or per-language-source-shape. It is *not* package
discovery, *not* registry lookup, *not* network calls.

### 3.2 Ecosystems — `normalize-ecosystems`

**Abstraction:** the `Ecosystem` trait
(`crates/normalize-ecosystems/src/lib.rs`). Each ecosystem (cargo, npm,
deno, python, go, hex, gem, composer, maven, nuget, nix, conan) is a unit
struct in `src/ecosystems/<name>.rs`, gated by a per-ecosystem feature.

**Scope of `Ecosystem`:**

- `name()`, `manifest_files()`, `lockfiles()`, `tools()` — detection.
- `fetch_info(query, tool)` — talk to the package manager / registry.
- `installed_version(package, project_root)` — read the lockfile.
- `list_dependencies(project_root)` — parse the manifest.
- `dependency_tree(project_root)` — resolve transitively via the tool.
- `published_names(project_root)` — names this project publishes.
- `audit(project_root)` — vulnerability scan via the tool.
- `query(package, project_root)` — convenience: detect tool + cache + fetch.

**Caching:** `query()` uses a 24-hour on-disk cache and falls back to stale
cache on network failure. Implementations of `fetch_info` should be
cache-unaware; the trait's default `query()` handles it.

**Critical boundary — ecosystem vs language:**

> Ecosystems resolve package coordinates (name@version) to **on-disk source
> paths or registry metadata**. Languages parse **source**. Neither alone
> is sufficient for whole-program work.

If your code parses Rust source for doc comments, it belongs in
`normalize-languages` (or a language-aware extractor crate), **not**
`normalize-ecosystems`. If your code calls `cargo metadata` to find where
`serde-1.0.228/` lives on disk, that is an ecosystem concern.

**Known violation, currently being repaid:** `normalize-ecosystems/src/local_docs.rs`
contains `CargoLocalDocsExtractor`, which both resolves source via
`cargo metadata` (ecosystem concern, fine) **and** parses Rust doc comments
from `.rs` files (language concern, wrong location). The language-parsing
half should move into `normalize-languages` or a new
`normalize-doc-extract` (TBD). See §10.

**`LocalDocsExtractor` vs `RemoteDocsFetcher`:** the two-trait coordinator
in `lib.rs` (`fetch_symbol_docs_with_fallback`) deliberately separates
local (no network) from remote (network) so the two paths can be tested,
cached, and reasoned about independently. Do not collapse them into one
trait "for simplicity". See §5.

**To add a new ecosystem** — see Extension Recipes §9.2.

### 3.3 Service layer — `crates/normalize/src/service/`

Each sub-service is a struct with `#[cli]`-annotated methods. To find the
template, look at `service/view.rs` or `service/docs.rs`.

**Adding a service method (= a CLI subcommand):**

0. **Check it doesn't already exist** under a different service. Commands
   have been moved before (`analyze ast` → `syntax ast`); a duplicate
   `analyze parse` was once added because nobody checked `syntax`.
1. **Decide where it lives.** If it belongs to an existing feature crate,
   add it there (the crate gets its own `#[cli]` service and is mounted by
   `NormalizeService`). Only put it in `service/` of the main crate if it's
   genuinely cross-cutting wiring with no home elsewhere.
2. Define the Report struct (`serde::Serialize + schemars::JsonSchema`) in
   the owning crate.
3. Implement `OutputFormatter` for the Report.
4. Add the `#[cli]` method to the owning service.
5. Mount the service in `NormalizeService::new` if it's a new crate-level
   service.
6. Add `assert_output_formatter::<NewReport>()` in the relevant `output.rs`
   test so the trait bound is checked at compile-time.

**`server-less` provides `--json`, `--jsonl`, `--jq` automatically** via
the `Serialize` derive. Do not add ad-hoc JSON support.

### 3.4 Facts and rules

The fact/rule stack has four layered crates. Read this section before
adding anything in this area; the naming is confusable.

```
normalize-facts-core            value types (Symbol, Import, Export, …)
normalize-facts                 extraction + SQLite store + parsers
normalize-facts-rules-api       Relations (Datalog inputs) + Diagnostic
normalize-facts-rules-interpret ascent-interpreter Datalog engine

normalize-syntax-rules          tree-sitter-query rule engine (.scm)
normalize-native-rules          pure-Rust checks (check-refs, ratchet, ...)
normalize-rules                 orchestration (run/list/show/enable/disable)
normalize-rules-config          shared TOML schema for all engines
```

**`normalize-facts-core`** is the vocabulary: `Symbol`, `SymbolKind`,
`Visibility`, `Import`, `Export`, `TypeRef`, `IndexedFile`. Used by
`normalize-facts` (storage), `normalize-languages` (extraction), and
`normalize-facts-rules-api` (analysis inputs). No I/O, no Tree-sitter,
no SQLite.

**`normalize-facts`** is extraction-into-storage. `SymbolParser` walks
tree-sitter ASTs with the language's `*.tags.scm` query and produces
`FlatSymbol`/`FlatImport` rows. `FileIndex` is the SQLite-backed store
(`.normalize/index.sqlite`).

**`normalize-facts-rules-api`** defines the `Relations` struct (the
Datalog input facts: `SymbolFact`, `CallFact`, `ImportFact`,
`ImplementsFact`, `IsImplFact`, `ParentFact`, …) and the `Diagnostic`
output. **Fact rules are interpreted `.dl` files** evaluated by
`normalize-facts-rules-interpret` over `Relations`. There is no dynamic
library loading for rule packs (the `abi_stable` cdylib pack mechanism
was dropped — see git history).

**`normalize-facts-rules-interpret`** is the ascent-interpreter bridge:
loads `.dl` files, populates input relations from the SQLite index,
runs the program, collects diagnostics. Built-in `.dl` rules live in
`src/builtin_dl/`.

**`normalize-syntax-rules`** runs **tree-sitter query** rules (`.scm`
patterns with TOML frontmatter), one source file at a time. This engine
covers the "lint a single file by pattern" case (no cross-file info,
no Datalog).

**`normalize-native-rules`** is a flat catalog of pure-Rust checks that
don't fit either of the above: `stale-doc`,
`check-examples`, `check-refs`, `ratchet`, `budget`,
`boundary-violations`, `high-complexity`, `high-fan-out`,
`high-fan-in`, `long-file`, `long-function`, `dead-parameter`. Each has
a `build_<name>_report` function returning a typed report.

**`normalize-rules`** is orchestration: `normalize rules run/list/show/
enable/disable/add/update/remove/tags`, and the `run_rules_report`
function called by `normalize ci`. It depends on all three engines and
applies the unified config from `normalize-rules-config` (severity
overrides, allow lists, `enabled = false`).

**Decision tree — where does my new rule go?**

- Pattern over one file's tree-sitter CST → `.scm` rule in
  `normalize-syntax-rules/src/builtin/` or a user `.scm` in
  `.normalize/rules/`.
- Cross-file or graph-shaped (callgraph, import cycles, dead exports) →
  `.dl` Datalog rule in `normalize-facts-rules-interpret/src/builtin_dl/`
  or a user `.dl` in `.normalize/rules/`. If you need a new input
  relation, add it to `Relations` in `normalize-facts-rules-api` and
  populate it from the index in `normalize-facts-rules-interpret`.
- Doesn't fit either (filesystem-shaped, mtime-sensitive, ratchet-style)
  → a new module in `normalize-native-rules` with a
  `build_<name>_report` function.

**Decision tree — fact vs index column:**

- The new info is per-symbol/per-import/per-call and rules will want to
  query it → add a new `…Fact` to `Relations` and a column to the
  corresponding SQLite table in `normalize-facts/src/index.rs`. Populate
  during extraction (`normalize-facts/src/extract.rs` calling into the
  Language trait).
- The info is one-off summary metadata → put it on `IndexedFile` or a
  scalar query in `normalize-facts`.

### 3.5 Knowledge graph — `normalize-knowledge-graph`

**Purpose:** persistent, queryable, agent-editable notes adjacent to code.
`.normalize/kg/<id>.md` is a Markdown file with YAML frontmatter; the
frontmatter holds `metadata` + `links` (outgoing typed edges). Three CLI
primitives (current API, 3-primitive design):

- `kg read <selector>` — selector → units (jq expression over frontmatter)
- `kg write <jq>` — jq transform to mutate or delete units
- `kg walk <start> <jq>` — BFS traversal using a jq expression to extract
  link targets

**Unit ID convention:** `[a-z0-9][a-z0-9-]*` (enforced by
`model::validate_id`). For machine-generated IDs (docs cache, fact-derived
units), build a deterministic slug:
`<source>-<language>-<package>-<version>-<symbol-slug>` is the convention
used by the docs cache (`docs-rust-serde-1-0-228-serde-serialize`).

**When to use KG vs the facts index:**

- KG: hand-authored or LLM-authored notes, design docs, decision records,
  cross-cutting links between symbols and concepts, **caches of expensive
  external lookups** (e.g. fetched documentation, embeddings). Append-only
  intent; concurrent edits resolve at frontmatter granularity.
- Facts index: machine-extracted, derivable-from-source, rebuilt by
  `normalize structure rebuild`. Don't store anything here you can't
  regenerate from disk.

**Cache pattern:** Caches in the KG should be written best-effort
(`let _ = write_unit(...)`) and read with explicit version pinning to avoid
returning stale data. See `crates/normalize/src/service/docs.rs:cache_write`
for the canonical pattern.

### 3.6 Context — `normalize-context`

**Not a knowledge graph. Not a config system.** It is a hierarchical
walker that resolves Markdown blocks from `.normalize/context/` (project
upward to filesystem root, then `~/.normalize/context/`). Each `.md` file
may have one or more YAML frontmatter blocks; blocks are filtered against
a `CallerContext` (flat dot-path map) using match strategies (`equals`,
`contains`, `keywords`, `regex`, `exists`, `one_of`, composable
`conditions: all:/any:`).

**Use it for:** per-project prompt fragments that LLM hooks inject when
their context matches (e.g. `claudecode.hook=UserPromptSubmit`).

**Do not use it for:** caching, persistent state, or anything that needs
edges/queries — that is the knowledge graph.

### 3.7 Tools — `normalize-tools`

**Purpose:** unified interface for external linters, formatters, type
checkers, and test runners — the things normalize *cannot* reimplement
(oxlint, eslint, ruff, prettier, black, rustfmt, biome, tsc, mypy,
pyright, cargo check, …).

**Pattern:** adapters in `src/adapters/<tool>.rs` and
`src/test_runners/<tool>.rs` implement a small trait, shell out to the
tool, normalise output (SARIF when the tool supports it, JSON otherwise)
into the shared diagnostic shape. The registry exposes
`default_registry()`, `registry_with_custom(root)` (reads
`.normalize/tools.toml`), and `ToolRegistry::with_builtins()`.

**Rule of thumb:** if the equivalent functionality could be implemented
natively in Rust with reasonable effort, do that and skip the adapter
(see "no shelling out" in CLAUDE.md hard constraints). Shell out when the
tool *is* the user's chosen workflow tool whose presence-and-version they
control (a project's eslint config, etc.).

## 4. Cross-cutting composition patterns

### 4.1 Ecosystem × Language composition

The most-misunderstood pattern in the codebase. Many features need
**both** ecosystem resolution and language parsing.

**Worked example — `normalize docs <symbol>`:**

To fetch documentation for `serde::Serialize`:

1. **Ecosystem layer (`Cargo` in `normalize-ecosystems`):** resolves
   `serde` to its on-disk source via `cargo metadata`, yielding
   `~/.cargo/registry/src/index.crates.io-.../serde-1.0.228/`.
2. **Language layer (`normalize-languages`, ideally):** opens
   `serde-1.0.228/src/lib.rs`, walks the module tree to find `Serialize`,
   extracts the attached doc comments using the Rust `Language` impl's
   docstring/attribute extraction.
3. **Coordinator (`fetch_symbol_docs_with_fallback`):** tries local; on
   any error, falls back to `DocsRsFetcher` (which fetches from
   docs.rs over HTTP).
4. **Service (`service/docs.rs`):** wraps the coordinator, handles
   version resolution (explicit → lockfile → latest), reads/writes the
   KG cache, and returns a `DocsReport`.

Neither layer alone is sufficient. The **resolution** is ecosystem-specific
(cargo's source layout differs from npm's `node_modules` differs from go's
`$GOMODCACHE`). The **parsing** is language-specific (Rust `///`,
JS/TS `/** */`, Python module docstrings).

**Where it currently lives wrong:** today both halves live in
`normalize-ecosystems/src/local_docs.rs`. This is the bug the feat
branch revealed — language parsing was put in the ecosystems crate. The
correct shape is for `normalize-languages` (or an extractor crate that
depends on it) to own the doc-comment-from-source walk, and
`normalize-ecosystems` to expose only `resolve_source_dir(package, version)
-> Result<PathBuf>`.

### 4.2 Index-first, single-file-fallback

Cross-file features (call graph, import resolution, dead-code, rule
engines that need `Relations`) require the SQLite index. Single-file
features (`view`, `syntax ast`, single-file complexity, single-file
syntax rules) work without it.

**Convention:** if a command needs the index and it's not built, **warn
and skip** (do not auto-build — slow in CI; do not error — non-actionable).
The user runs `normalize structure rebuild`. See
`service/mod.rs::NormalizeService::ci` for the canonical pattern.

### 4.3 Daemon transparency

The background daemon (`normalize daemon`) caches index state, rule
results, and the context index in memory and pushes events over a Unix
domain socket. **Commands should not require the daemon.** Every command
must work daemon-less, falling back to direct SQLite reads and on-demand
parses. The daemon is an optimisation, not a dependency.

## 5. Local-first, remote-fallback

**Principle:** when normalize has access to local data (cargo registry
source, `node_modules/`, git history, the SQLite index, the KG cache),
it must prefer that over network calls. Network calls are slow, flaky,
and silently rate-limited; they belong only on the fallback path.

**Typical structure:**

```rust
pub trait LocalThing { fn resolve(&self, …) -> Result<T, E>; }
pub trait RemoteThing { fn fetch(&self, …) -> Result<T, E>; }

pub fn get_with_fallback(local: &dyn LocalThing, remote: &dyn RemoteThing, …)
    -> Result<T, E>
{
    match local.resolve(…) {
        Ok(v) => Ok(v),
        Err(local_err) => remote.fetch(…)
            .map_err(|remote_err| combine(local_err, remote_err)),
    }
}
```

Reference implementation:
`normalize_ecosystems::fetch_symbol_docs_with_fallback` — coordinates
`LocalDocsExtractor` and `RemoteDocsFetcher`. Both traits are
single-method (`extract_docs` / `fetch_docs`) and intentionally not
collapsed.

**Why two traits, not one with a `bool local_only` flag:**

- Independent test surfaces (mock one without the other).
- Independent cache policies (local is "as fresh as disk"; remote needs
  a TTL).
- Independent error taxonomies (a network error from `local` is a bug;
  a "not in lockfile" from `remote` is a bug).
- Independent impl sites: a new ecosystem can land remote-only or
  local-only first.

**Anti-pattern:** a single `DocsExtractor` trait with implementations
that "sometimes hit the network, sometimes don't" depending on flags. Do
not do this.

## 6. Anti-patterns observed

Concrete things that have been done wrong (some still in the tree); do
not replicate.

- **Language-parsing logic in `normalize-ecosystems`.** See
  `local_docs.rs`. Ecosystems resolve; languages parse.
- **A trait that conflates local-on-disk and remote-network paths.**
  Always split into two traits + a coordinator (§5).
- **New top-level crate when an existing one fits.** Per-language code →
  `normalize-languages`. Per-ecosystem code → `normalize-ecosystems`.
  Per-native-check → `normalize-native-rules`. Per-tool adapter →
  `normalize-tools`.
- **Network call when local data exists.** Cargo source is already on
  disk; npm packages are in `node_modules/`; git history is in `.git/`.
  Read locally first.
- **Bypassing `#[cli]`.** Don't add hand-rolled clap dispatch. The
  legacy paths in `main.rs` are migration debt, not a template.
- **Language-specific branches in language-agnostic crates.** If you
  find yourself writing `if grammar_name == "rust"` in
  `normalize-syntax-rules`, the query belongs in
  `normalize-languages/src/queries/`.
- **Runner-wide filters that override every rule.** Filtering decisions
  go on the rule (a metadata field the runner consults), not on the
  runner.
- **Hardcoded third-party-tool conventions.** `node_modules/`,
  `target/`, `.venv/` etc. are conventions of normalize *consumers*.
  They belong in project config, not as constants in
  `normalize-native-rules` or `normalize-syntax-rules`.
- **Reading mutable globals at call sites.** Capture env vars and config
  once in a constructor; pass dependencies in.
- **Shelling out when a Rust crate exists.** Use `gix` not `git`,
  `fast_rsync` not `rsync`. Exception: tools that are part of the user's
  chosen workflow.
- **Stub implementations.** Returning `None` / empty is only correct
  when the concept genuinely doesn't exist in that language. Don't
  fabricate semantic structure to "fill in" a Language trait method
  whose grammar doesn't model the concept.
- **Returning `String` from a service method.** Return a typed Report.
- **Roadmaps under `docs/`.** Active plans go in `TODO.md`. `docs/` is
  for stable reference material only.

## 7. Conventions cheat sheet

### Naming
- No crate prefix beyond `normalize-`. Crate names must be available on
  crates.io.
- Crate suffixes:
  - `-core`: shared value types / vocabulary depended on by multiple
    crates in the same area (e.g. `normalize-facts-core`,
    `normalize-core`). No I/O, no heavy deps.
  - `-api`: stable types defining a boundary between an engine and its
    consumers (e.g. `normalize-facts-rules-api` defines what fact rules
    see).
  - `-interpret` / `-config` / `-meta`: capability suffix when the crate
    is one face of a multi-crate area.
- All public output structs end in `Report` (not `Result`):
  `ViewReport`, `DocsReport`, `RulesListReport`, `CiReport`.

### File layout per crate
- `Cargo.toml`, `src/lib.rs`, optional `src/main.rs`, optional
  `src/service.rs` / `src/service/` behind `cli` feature, optional
  `src/bin/`.

### Error type conventions
- Per-crate `Error` enum derived `Debug` + `Display` + `std::error::Error`
  (manual impls — no `thiserror` blanket adoption yet). See
  `PackageError`, `DocsError`, `IndexError` for examples.
- `From` impls between sibling error types in the same area when one
  semantically wraps the other (`PackageError → DocsError`).
- Service methods return `Result<Report, String>` at the CLI boundary —
  the string is what server-less will render as the error message.

### Async vs sync
- The CLI dispatches into async at the top level (`tokio` runtime); most
  service methods are `async fn`.
- Heavy CPU-bound work (rule running, parsing) goes through
  `tokio::task::spawn_blocking` to avoid stalling the reactor — see
  `service/mod.rs::ci` for the canonical pattern.
- Library crates expose blocking APIs by default; the daemon and the
  service layer wrap them.

### Imports / re-exports
- Crates re-export their own public types at the crate root (`pub use`
  in `lib.rs`). Consumers should not reach into private modules.
- Shared vocabulary is re-exported from the `-core` crate by every
  consumer (`pub use normalize_facts_core::{Symbol, …}`) so callers
  don't depend on `normalize_facts_core` directly unless they need it.

## 8. Glossary

- **Unit** — a knowledge graph node. Markdown file with YAML frontmatter
  (`metadata` + `links`) and body. ID grammar `[a-z0-9][a-z0-9-]*`.
- **Edge / Link** — directed typed connection between Units. *Link* is
  the storage form (in the source unit's frontmatter); *Edge* is the
  projected form (used in query results, with both endpoints).
- **Fact** — a row in `Relations`, the Datalog input set. E.g. a
  `SymbolFact { file, name, kind, line }` or a `CallFact { caller_file,
  caller_name, callee_name, line }`. Extracted from the SQLite index.
- **Rule** — a check that produces `Diagnostic`s. Three flavours:
  *syntax rule* (`.scm` query + TOML frontmatter), *fact rule* (`.dl`
  Datalog program), *native rule* (Rust function in
  `normalize-native-rules`).
- **Finding / Issue / Diagnostic** — interchangeable in the codebase for
  a single rule violation. The canonical structured form is
  `normalize_output::diagnostics::Issue`.
- **Ecosystem** — a package manager + its conventions. `Cargo`, `Npm`,
  `Go`, etc. Detects via manifest/lockfile, resolves package
  coordinates, calls registry / metadata tools.
- **Language** — a programming language as seen through its tree-sitter
  grammar plus our `Language` impl. ~98 of them, all zero-sized structs.
- **Resolution** — turning an import specifier (e.g.
  `use serde::Serialize;`, `from foo import bar`) into a concrete file
  path + exported name. Done by per-language `ModuleResolver` impls.
- **Index** — `.normalize/index.sqlite`, the persistent fact store.
- **KG / Knowledge Graph** — `.normalize/kg/`, the unit + edge store.
  Distinct from the index.
- **Service** — a `#[cli]`-annotated impl block exposing CLI subcommands.
  `NormalizeService` is the root; sub-services live in `service/<area>.rs`
  or in their own crate.
- **Report** — typed output of a service method. Implements
  `Serialize + JsonSchema + OutputFormatter`.
- **Shadow git** — separate git store under `.normalize/shadow/` tracking
  edit history independently of the user's git. See `normalize-shadow`.

## 9. Extension recipes

### 9.1 Add a new language

1. Verify the grammar is in arborium (`xtask` or
   `normalize-grammars` will tell you). If not, write the grammar — do
   not pull a random tree-sitter crate from the ecosystem.
2. `crates/normalize-languages/src/<lang>.rs`: implement `Language`.
   Zero-sized struct + trait impl. Use existing simple languages as
   templates (`zsh.rs`, `fish.rs`).
3. Add `.scm` query files under
   `crates/normalize-languages/src/queries/`: at minimum `*.tags.scm`;
   add `*.imports.scm`, `*.calls.scm`, `*.complexity.scm`, `*.types.scm`,
   `*.decorations.scm`, `*.cfg.scm` as the grammar supports. Use
   `normalize syntax ast <file>` to discover real node kinds; do not
   guess.
4. Wire feature flag in `Cargo.toml` (`lang-<name>`), add to a langs-*
   group flag.
5. Register the struct via `pub mod <lang>` + `pub use <lang>::<Type>`
   in `lib.rs`.
6. Add `Capabilities` entry in `normalize-language-meta` if the
   language has non-default capability shape.
7. If the language has a module system, add a `ModuleResolver` impl and
   return it from `Language::module_resolver()`.
8. Run `cargo test -q` and `normalize ci`.

### 9.2 Add a new ecosystem

1. `crates/normalize-ecosystems/src/ecosystems/<eco>.rs`: implement
   `Ecosystem`. Unit struct + trait impl.
2. Wire feature flag in `Cargo.toml` (`<eco>` — note: no `eco-` prefix).
3. Add `pub mod <eco>` + `pub use <eco>::<Type>` in
   `ecosystems/mod.rs` (feature-gated).
4. Register in `init_builtin()` in `ecosystems/mod.rs`.
5. Add manifest and lockfile entries to `manifest_files()` and
   `lockfiles()` so detection works.
6. Tests: at minimum, detection roundtrip + `installed_version` against
   a fixture lockfile.

### 9.3 Add a new service method (= CLI subcommand)

1. Check it doesn't already exist under another service
   (`normalize --help`).
2. Decide owning crate per §2. If new crate is needed, create it with
   its own `cli` feature and a `service.rs`.
3. Define `<Verb>Report` with `serde::Serialize + schemars::JsonSchema`.
4. `impl OutputFormatter for <Verb>Report`.
5. Add the `#[cli]` method to the owning service. Annotate args with
   `#[param(…)]`. Use `display_with = "display_output"` to bridge to
   `OutputFormatter`.
6. Add `assert_output_formatter::<<Verb>Report>()` in the relevant
   `output.rs` test.
7. If a new crate-level service, mount it in `NormalizeService::new`
   and add a delegating method in the `#[cli]` impl block of
   `NormalizeService`.
8. `cargo clippy --all-targets --all-features -- -D warnings && cargo test -q`.
9. Update `CHANGELOG.md` under `[Unreleased]`.

### 9.4 Add a new fact / Datalog rule

1. Decide: pattern-on-one-file → syntax rule; cross-file/graph → fact
   rule; mtime-or-filesystem-shape → native rule. See §3.4 decision tree.
2. **Fact rule:** write `.dl` under
   `normalize-facts-rules-interpret/src/builtin_dl/<rule-id>.dl` with
   TOML frontmatter (`id`, `message`, `enabled`, severity). Reference
   the input relations enumerated in
   `normalize-facts-rules-interpret/src/lib.rs` (top of file).
3. If you need a relation that doesn't exist:
   - Add the `…Fact` struct to `normalize-facts-rules-api/src/relations.rs`.
   - Add a column / table to `normalize-facts/src/index.rs`.
   - Populate during extraction in `normalize-facts/src/extract.rs`,
     calling whatever `Language` trait method you need (extend the
     trait if necessary).
   - Wire population in `normalize-facts-rules-interpret`'s relation
     loader.
4. Add a fixture test under `normalize-facts-rules-interpret/src/tests.rs`.
5. Document the rule in `docs/rules/<id>.md`.

### 9.5 Add a new MCP-exposed capability

Every `#[cli]` service method is automatically an MCP tool when the user
runs `normalize serve mcp`. There is **no separate MCP wiring**. If you
want a capability to be MCP-visible:

1. Make sure it's a `#[cli]` method (not legacy clap dispatch).
2. Make sure the args have decent `#[param(help = "…")]` strings — they
   become the MCP tool's parameter descriptions.
3. The doc-comment on the method becomes the MCP tool description; write
   it for an LLM caller.
4. Group it sensibly with `#[server(group = "…")]` (`core`, `analysis`,
   `utilities`, `infrastructure`).

## 10. In flight / known incomplete

This section is intentionally short and intentionally cited — `TODO.md`
is the live source of truth, this is just the pointer.

### `feat/docs-into-context-mvp` branch

Adds `normalize docs <symbol>` — fetch upstream symbol docs into LLM
context. As of writing:

- `Cargo`-only on the ecosystem side. Other ecosystems use the
  default-`Err` `fetch_symbol_docs` impl.
- Two-trait coordinator (`LocalDocsExtractor` + `RemoteDocsFetcher`)
  with `fetch_symbol_docs_with_fallback` lives in
  `normalize-ecosystems/src/lib.rs`. This is the right shape.
- **`CargoLocalDocsExtractor` in `normalize-ecosystems/src/local_docs.rs`
  is misplaced.** The `cargo metadata` source-resolution half is
  ecosystem (correct), but the doc-comment parsing of `.rs` files is a
  language concern that belongs in `normalize-languages` (or a new
  extractor crate that depends on it). The remaining work:
  - Extract a `resolve_source_dir(package, version, project_root) ->
    Result<PathBuf, DocsError>` from `local_docs.rs` and keep it in
    `normalize-ecosystems` (or add `Ecosystem::resolve_source_dir`).
  - Move the doc-comment walk into `normalize-languages` as a
    `Language`-trait method (e.g. `extract_symbol_docs(src: &str,
    symbol_path: &[&str]) -> Option<SymbolDoc>`) or a new trait
    `LanguageDocs` opted into via `Language::as_docs()`.
  - Update the coordinator to take both pieces by reference.
- Other unfinished work tracked in `TODO.md` under "docs" / "feat".

### General

- `Architecture.md` (lowercase) predates this document and overlaps
  significantly. Plan: keep it as the high-level CLI overview; have
  this file own the contributor-facing architectural contract. Resolve
  the two-doc split when the lowercase file next needs updates.
- Some legacy clap dispatch in `crates/normalize/src/main.rs` remains
  un-migrated to `#[cli]`. New work goes through `#[cli]` only.
