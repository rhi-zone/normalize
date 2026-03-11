# Normalize Philosophy

This document contains the design philosophy and architectural overview for Normalize.
For behavioral rules and conventions, see `CLAUDE.md`.

## Project Overview

Normalize is **structural code intelligence as a platform**. It provides tools for understanding, navigating, and modifying code at a structural level (AST, control flow, dependencies) rather than treating code as text.

| User | Interface | Use Case |
|------|-----------|----------|
| Developer | CLI | Understand unfamiliar code, explore structure |
| CI/CD | CLI | Quality gates, validation, linting |
| Tool Builder | Library | Build custom tools on structural primitives |
| Agent | CLI + JSON | Code intelligence for LLMs and automated workflows |

LSP integration is a future direction — the groundwork (symbol extraction, scope analysis, references) is in place, but it's not a current interface.

## Scope

Normalize started as three composable primitives (view, edit, analyze) and grew into a code intelligence platform with ~22 top-level commands and 90+ entry points across a 38-crate workspace.

The CLI surface today covers:

| Domain | Commands | Examples |
|--------|----------|----------|
| Viewing & navigation | `view`, `grep`, `context` | File/symbol browsing, search, context extraction |
| Editing | `edit` | Structural code modifications |
| Analysis | `analyze` (40+ subcommands) | Complexity, duplication, coupling, test gaps, architecture |
| Linting & rules | `rules` | Multi-engine rule system (syntax, fact, native, SARIF) |
| Indexing | `structure` | Cross-file fact extraction (symbols, imports, calls) |
| Package management | `package` | Dependency info, audit, outdated, tree |
| Code generation | `generate`, `translate` | OpenAPI clients, type generation, syntax translation |
| Developer tooling | `sessions`, `daemon`, `grammars`, `init` | Session analysis, background services, grammar management |
| Introspection | `syntax`, `aliases`, `config` | Tree-sitter inspection, filter aliases, configuration |
| Servers | `serve` | MCP, HTTP, LSP interfaces |

The architectural principles below still hold — the system grew by applying them, not by abandoning them. View/edit/analyze remain the conceptual core, with supporting infrastructure built around them.

## Architecture

Normalize is a structural codebase analysis toolkit. It understands code at the CST level via tree-sitter, extracts facts (symbols, imports, calls, complexity) into an index, and exposes them through commands organized around three conceptual operations: `view` (read/navigate), `edit` (structural modification), and `analyze` (compute properties), plus supporting infrastructure for linting, indexing, package management, and developer tooling.

The index enables cross-file features (import resolution, dead code detection, dependency graphs). Single-file features — viewing, complexity, parsing — degrade gracefully without it. The index is not a prerequisite, but cross-file intelligence requires it.

---

## Design Tenets

### Core Philosophy

#### Generalize, Don't Multiply

When facing N use cases, prefer one flexible solution over N specialized ones. Composability reduces cognitive load, maintenance burden, and token cost.

Examples:
- Core operations (view, edit, analyze) with rich options, plus domain-grouped supporting commands — not 100 unrelated specialized tools
- Log formats as plugins, not N hardcoded parsers
- `--json` + `--jq` + `--pretty` flags, not `--format=X` for every format
- Distros that compose, not fork

Complexity grows linearly with primitives, exponentially with combinations. A system with 3 composable primitives is simpler than one with 30 specialized tools, even if the 30-tool system has less code per tool.

#### Separate Interface, Unify Plumbing

The user-facing interface should reflect intent (view, edit, analyze are different actions). The implementation should share machinery (path resolution, filtering, node targeting, output formatting).

This gives us:
- **Clarity**: Users know what operation they're performing
- **Safety**: Read vs write intent is explicit
- **DRY**: One path resolver, one filter implementation, one tree model
- **Consistency**: Same `--type`, `--calls`, `--deps` flags work everywhere

The interface serves the user's mental model. The plumbing stays unified.

#### Structure Over Text

Return structure, not prose. Structured data composes; text requires parsing.

Hierarchy implies trees. Code (AST), files (directories), tasks (subtasks), agents (sub-agents)—all trees. Design operations that work on trees: prune, query, navigate, transform. When something isn't a tree (call graphs, dependencies), it's a tree with cross-links.

Few orthogonal primitives beat many overlapping features. Lua got this right with tables. Find the smallest set of operations that compose well, not the largest set of features that cover cases.

#### Unified Codebase Tree

The codebase is one tree. Filesystem and AST are not separate—they're levels of the same hierarchy:

```
project/                    # root
├── src/                    # directory node
│   ├── main.py             # file node
│   │   ├── class Foo       # class node
│   │   │   └── bar()       # method node
│   │   └── def helper()    # function node
```

Uniform addressing with `/` everywhere:
- `src/main.py/Foo/bar` - method `bar` in class `Foo` in file `main.py`
- Resolution uses filesystem as source of truth: check if each segment is file or directory
- No ambiguity: can't have file and directory with same name in same location
- Accept multiple separators for familiarity, normalize internally:
  - `/` (canonical): `src/main.py/Foo/bar`
  - `::` (Rust-style): `src/main.py::Foo::bar`
  - `:` (compact): `src/main.py:Foo.bar`
  - `#` (URL fragment): `src/main.py#Foo.bar`

Same primitives work at every level.

#### Conceptual Primitives

The CLI is organized around three core operations, with supporting commands built around them:

| Operation | Purpose | Composable options |
|-----------|---------|-------------------|
| `view` | See/find nodes | `--depth`, `--deps`, `--type`, `--calls`, `--called-by` |
| `edit` | Modify a node | `--insert`, `--replace`, `--delete`, `--move` |
| `analyze` | Compute properties | `--health`, `--complexity`, `--security` |

These aren't the only commands — `rules`, `structure`, `grep`, `sessions`, and others exist as first-class entry points. But view/edit/analyze remain the conceptual core: understand code, modify code, measure code. Supporting commands (linting, indexing, configuration) exist to make those three operations richer.

Depth controls expansion: `view src/ --depth 2` shows files and their top-level symbols. Filters compose: `view --type function --calls "db.*"` finds functions that call database code.

Discoverability through structure. The CLI surface is larger than three commands, but the mental model stays simple: what am I trying to do — see something, change something, or measure something? Supporting commands are organized by domain (rules, grammars, sessions) so they're findable without memorizing an inventory.

Put smarts in the tool, not the schema. Tool definitions cost context. A few well-understood entry points with composable options beat a flat list of specialized tools.

#### Friction Minimization Loop

Normalize's meta-goal: make it easier to reduce friction, which accelerates development, which makes it easier to improve normalize.

```
Workflows documented → Failure modes identified → Encoded as tooling → Friction reduced → Faster iteration → (loop)
```

Prevention hierarchy (most to least reliable):
1. **Tooling** - automated checks catch it (linters, CI, rules)
2. **Type system** - invalid states unrepresentable
3. **Tests** - regression tests encode invariants
4. **Process** - workflow steps force checks
5. **Documentation** - requires someone to read (unreliable)

The goal is moving failure modes UP this hierarchy. Documentation is the fallback when tooling doesn't exist yet, not the primary defense. Every documented failure mode is a candidate for automation.

#### Err on Keeping Data

Disk is cheap. Lost work is expensive. When in doubt, preserve:
- History should be additive (undo creates branches, doesn't destroy)
- Deletions should be recoverable (shadow git, trash, soft delete)
- Prune explicitly, not automatically
- Default to archaeology over cleanup

This doesn't mean keep everything forever—it means destruction requires intent, not accident.

---

### User Experience

#### Defaults & Onboarding

**Maximally useful defaults**: Every configurable option should have a default that:
- Works well for the common case (80% of users shouldn't need to configure it)
- Errs on the side of usefulness over safety-theater
- Can be discovered and changed when needed

**Low barrier to entry**:
- Works out of the box with minimal configuration
- Sensible defaults for common workflows
- Progressive disclosure: simple things simple, complex things possible
- Clear error messages that guide users toward solutions

**Fast specialization**: Good defaults mean acceptable general performance out of the box. But we should absolutely support hyper-specialization:
- **Quick wins first**: Default config should "just work" reasonably well
- **Escape hatches**: When defaults aren't enough, specialization should be one step away
- **Zero-to-custom fast**: The path from "using defaults" to "fully customized" should be short and obvious
- **No ceiling**: Power users shouldn't hit walls. If someone wants to optimize for their exact workflow, let them

This is a conscious tradeoff: defaults optimize for breadth (works for everyone), specialization optimizes for depth (works perfectly for you). Both are valid, and the system should excel at both ends of the spectrum.

#### Scriptability

Every interactive feature must have a non-interactive equivalent. Scripts and CI/CD need deterministic behavior with no prompts.

- Interactive prompts → flags that specify the choice upfront
- `--hunk` (interactive selection) → `--hunk-id h1,h3` or `--lines 10-25`
- Confirmation prompts → `--yes` / `--no` flags
- Prompts with defaults → flags that match default behavior

If a human can do it interactively, automation should be able to do it non-interactively.

#### Error Recovery & Affordances

**Forgiving lookups**: Users make mistakes—typos, wrong conventions, forgotten paths. Every lookup should be forgiving:
- Fuzzy file resolution: `prior_art` finds `prior-art.md`
- Symbol search tolerates partial names and typos
- Pattern: try exact → try fuzzy → try corrections → ask for clarification

Note: The conceptual model (view/edit/analyze) reduces tool selection ambiguity. This principle applies to path and symbol resolution within commands.

**Suggest obvious corrections**: When something seems wrong, suggest the likely fix. Not "here's what you could do" (overwhelming) but "did you mean X?" (helpful).
- Symbol not found → "Did you mean: `normalize grep 'foo' file.rs`"
- File not found → suggest fuzzy matches or similar names
- Operation failed → suggest the recovery action

**Report what was done**: After mutations, show what changed so users can validate nothing unexpected happened.
- Files changed, lines added/removed
- Summary matches what `--dry-run` would show
- Especially important when the effect isn't obvious from the command
- Enables quick "undo if wrong" decisions

The goal: users should never wonder "did that work?" or "what did that do?"

#### Works Anywhere

**Messy codebases**: Real-world code is often messy. Normalize should:
- Handle legacy code without requiring refactoring first
- Degrade gracefully when AST parsing fails (text fallbacks)
- Support incremental improvement (clean up as you go, or don't)
- Not impose architectural opinions unless asked

**Just work, then customize**: Tools should work immediately on whatever users already have:
- Parse common formats without configuration (TODO.md, CHANGELOG, configs)
- Handle variations gracefully (checkboxes, numbers, bullets, headers)
- Never require users to restructure files to match tool expectations
- Detect patterns, don't mandate them

When structure is ambiguous, make a reasonable choice. When truly unclear, ask—but aim for that to be rare. The goal: zero configuration for 90% of cases, explicit config for edge cases.

---

### Implementation

#### Architecture

**Hyper-modular**: Prefer many small, focused modules over fewer large ones:
- Maintainability: Easier to understand, modify, and test small units
- Composability: Small pieces combine flexibly
- Refactorability: Can restructure without rewriting everything

The 38-crate workspace reflects this: domain logic lives in focused crates (`normalize-languages`, `normalize-facts`, `normalize-rules`, etc.), with the main `normalize` crate handling CLI wiring and service dispatch. The main crate is larger than ideal — extraction of analysis algorithms into domain crates is ongoing.

**Library-first**: Domain crates should be reusable Rust libraries. The main crate (`crates/normalize/`) is the CLI consumer, not the home for reusable logic.

**Everything is a plugin**: Where possible, use plugin protocols instead of hardcoded implementations. Even "native" integrations should implement the same plugin interface as third-party ones.

#### Resource Efficiency

Normalize should be extremely lightweight. High memory usage is a bug:
- **Low RAM footprint**: Favor streaming and lazy loading over large in-memory caches
- **Transparent metrics**: Every command should optionally show RAM and timing breakdowns

---

## Meta

Nothing good appears from scratch. Iterate. CLAUDE.md grew through 20+ commits, not upfront investment. Features emerge from use, not design documents. Start minimal, capture what you learn, repeat.
